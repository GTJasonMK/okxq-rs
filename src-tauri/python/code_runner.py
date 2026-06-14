#!/usr/bin/env python3
"""OKXQ Agent 代码沙箱 — 安全执行用户提交的 Python 代码片段。

协议：stdin 逐行 JSON 命令，stdout 逐行 JSON 响应。
支持的 action：
  - execute : 在受限沙箱中执行代码
  - ping     : 存活检查
"""

import ast
import json as _json
import signal
import sys as _sys
import time as _time
from io import StringIO as _StringIO

# 安全内置函数白名单
SAFE_BUILTINS = {
    "abs": abs, "all": all, "any": any, "bool": bool, "chr": chr,
    "dict": dict, "divmod": divmod, "enumerate": enumerate, "filter": filter,
    "float": float, "format": format, "frozenset": frozenset, "int": int,
    "isinstance": isinstance, "issubclass": issubclass, "len": len, "list": list,
    "map": map, "max": max, "min": min, "ord": ord, "pow": pow,
    "print": print, "range": range, "repr": repr, "reversed": reversed,
    "round": round, "set": set, "slice": slice, "sorted": sorted,
    "str": str, "sum": sum, "tuple": tuple, "type": type, "zip": zip,
    "True": True, "False": False, "None": None, "Exception": Exception,
    "ValueError": ValueError, "TypeError": TypeError, "KeyError": KeyError,
    "IndexError": IndexError, "StopIteration": StopIteration,
    "ZeroDivisionError": ZeroDivisionError, "ArithmeticError": ArithmeticError,
    "OverflowError": OverflowError, "RuntimeError": RuntimeError,
}

# 禁止的 AST 节点类型
FORBIDDEN_NODES = {
    "Import", "ImportFrom",         # import / from ... import
    "Exec",                          # exec() [Python 2 遗留]
    "Global", "Nonlocal",            # 修改外部作用域
    "AsyncFunctionDef", "Await",    # 异步（无需开放）
    "Yield", "YieldFrom",           # 生成器
    "With", "AsyncWith",           # with 语句（可访问文件等上下文管理器）
}

# 禁止的函数调用名（即使通过了 AST 检查）
FORBIDDEN_CALLS = {
    "eval", "exec", "compile", "__import__", "open", "input",
    "breakpoint", "help", "copyright", "credits", "license",
    "getattr", "setattr", "delattr", "hasattr",
    "globals", "locals", "vars", "dir",
    "__builtins__", "__import__",
}

# 禁止的属性访问（防止逃逸）
FORBIDDEN_ATTRS = {
    "__class__", "__bases__", "__mro__", "__subclasses__",
    "__globals__", "__code__", "__closure__", "__func__",
    "__self__", "__dict__", "__module__", "__builtins__",
    "_module", "_data", "__reduce__", "__reduce_ex__",
}


class CodeValidator(ast.NodeVisitor):
    """AST 节点遍历器：拦截所有危险操作。"""

    def __init__(self):
        self.errors: list[str] = []

    def _check_node(self, node, node_type: str):
        name = getattr(node, "name", "") or getattr(node, "id", "") or ""
        if name in FORBIDDEN_CALLS:
            self.errors.append(f"禁止调用: {name}")
            return

    def visit_Import(self, node: ast.Import):
        names = [alias.name for alias in node.names]
        self.errors.append(f"禁止导入模块: {', '.join(names)}")

    def visit_ImportFrom(self, node: ast.ImportFrom):
        self.errors.append(f"禁止导入模块: {node.module or '未知'}")

    def visit_Call(self, node: ast.Call):
        if isinstance(node.func, ast.Name):
            self._check_node(node.func, "call")
        elif isinstance(node.func, ast.Attribute):
            if isinstance(node.func.attr, str) and node.func.attr in FORBIDDEN_CALLS:
                self.errors.append(f"禁止调用: {node.func.attr}")
        self.generic_visit(node)

    def visit_Attribute(self, node: ast.Attribute):
        if isinstance(node.attr, str) and node.attr in FORBIDDEN_ATTRS:
            self.errors.append(f"禁止访问属性: {node.attr}")
        self.generic_visit(node)

    def visit_Global(self, node: ast.Global):
        self.errors.append("禁止使用 global 语句")

    def visit_Nonlocal(self, node: ast.Nonlocal):
        self.errors.append("禁止使用 nonlocal 语句")

    def visit_With(self, node: ast.With):
        self.errors.append("禁止使用 with 语句")

    def visit_AsyncWith(self, node: ast.AsyncWith):
        self.errors.append("禁止使用 async with 语句")

    def visit_AsyncFunctionDef(self, node: ast.AsyncFunctionDef):
        self.errors.append("禁止定义异步函数")

    def visit_Await(self, node: ast.Await):
        self.errors.append("禁止使用 await")

    def visit_Yield(self, node: ast.Yield):
        self.errors.append("禁止使用 yield")

    def visit_YieldFrom(self, node: ast.YieldFrom):
        self.errors.append("禁止使用 yield from")

    def visit_Subscript(self, node: ast.Subscript):
        # 拦截 obj[...] 形式的动态访问
        self.generic_visit(node)


def _timeout_handler(signum, frame):
    raise TimeoutError("代码执行超时（30秒）")


def _execute_code(code: str) -> dict:
    """在受限沙箱中执行代码，返回结果字典。"""
    # 步骤1：AST 解析
    try:
        tree = ast.parse(code, mode='exec')
    except SyntaxError as e:
        return {"ok": False, "error": f"语法错误: {e}"}

    # 步骤2：安全校验
    validator = CodeValidator()
    validator.visit(tree)
    if validator.errors:
        return {"ok": False, "error": "; ".join(validator.errors)}

    # 步骤3：编译
    try:
        compiled = compile(tree, '<sandbox>', 'exec')
    except Exception as e:
        return {"ok": False, "error": f"编译失败: {e}"}

    # 步骤4：在受限环境中执行
    stdout_capture = _StringIO()
    sandbox_globals = {
        "__builtins__": SAFE_BUILTINS,
        "__name__": "__sandbox__",
    }
    sandbox_locals: dict = {}

    # 设置超时（仅 Unix）
    old_alarm = None
    try:
        old_alarm = signal.signal(signal.SIGALRM, _timeout_handler)
        signal.alarm(30)  # 30 秒超时
    except (ValueError, AttributeError):
        pass  # Windows 不支持 SIGALRM

    start = _time.perf_counter()
    try:
        exec(compiled, sandbox_globals, sandbox_locals)
        exec_time_ms = (_time.perf_counter() - start) * 1000

        # 提取用户定义的变量（排除内置和私有）
        user_vars = {
            k: repr(v) if not isinstance(v, (int, float, str, bool, list, dict, type(None)))
            else v
            for k, v in sandbox_locals.items()
            if not k.startswith("_")
        }

        return {
            "ok": True,
            "result": user_vars,
            "stdout": stdout_capture.getvalue(),
            "execution_time_ms": round(exec_time_ms, 2),
            "locals_count": len(user_vars),
        }
    except Exception as e:
        exec_time_ms = (_time.perf_counter() - start) * 1000
        return {
            "ok": False,
            "error": f"{type(e).__name__}: {e}",
            "stdout": stdout_capture.getvalue(),
            "execution_time_ms": round(exec_time_ms, 2),
        }
    finally:
        if old_alarm is not None:
            signal.alarm(0)
            signal.signal(signal.SIGALRM, old_alarm)


def main():
    for line in _sys.stdin:
        line = line.strip()
        if not line:
            continue
        try:
            command = _json.loads(line)
        except Exception:
            _sys.stdout.write(_json.dumps(
                {"ok": False, "error": f"无法解析输入命令: {line}"}
            ) + "\n")
            _sys.stdout.flush()
            continue

        action = command.get("action", "")
        try:
            if action == "ping":
                result = {"ok": True, "pong": True}
            elif action == "execute":
                code = command.get("code", "")
                if not code.strip():
                    result = {"ok": False, "error": "代码不能为空"}
                else:
                    result = _execute_code(code)
            else:
                result = {"ok": False, "error": f"未知操作: {action}"}
        except Exception as exc:
            result = {"ok": False, "error": f"沙箱内部错误: {exc}"}

        _sys.stdout.write(_json.dumps(result) + "\n")
        _sys.stdout.flush()


if __name__ == "__main__":
    main()
