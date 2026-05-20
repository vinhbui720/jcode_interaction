#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import importlib
import pathlib
import traceback

ROOT = pathlib.Path(__file__).resolve().parent
TEST_DIR = ROOT / "tests"


class MonkeyPatch:
    def __init__(self) -> None:
        self._undo = []

    def setattr(self, target: str, value) -> None:
        parts = target.split(".")
        module = None
        module_index = 0
        for index in range(len(parts), 0, -1):
            module_name = ".".join(parts[:index])
            try:
                module = importlib.import_module(module_name)
            except ModuleNotFoundError:
                continue
            module_index = index
            break
        if module is None or module_index >= len(parts):
            raise AttributeError(target)
        obj = module
        for attr in parts[module_index:-1]:
            obj = getattr(obj, attr)
        attr = parts[-1]
        old = getattr(obj, attr)
        setattr(obj, attr, value)
        self._undo.append((obj, attr, old))

    def undo(self) -> None:
        while self._undo:
            module, attr, old = self._undo.pop()
            setattr(module, attr, old)


def main() -> int:
    total = 0
    failed = 0
    for path in sorted(TEST_DIR.glob("test_*.py")):
        spec = importlib.util.spec_from_file_location(path.stem, path)
        assert spec and spec.loader
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        for name in sorted(dir(module)):
            if not name.startswith("test_"):
                continue
            fn = getattr(module, name)
            if not callable(fn):
                continue
            total += 1
            try:
                argnames = fn.__code__.co_varnames[: fn.__code__.co_argcount]
                kwargs = {}
                monkeypatch = None
                if "monkeypatch" in argnames:
                    monkeypatch = MonkeyPatch()
                    kwargs["monkeypatch"] = monkeypatch
                if "tmp_path" in argnames:
                    import tempfile
                    with tempfile.TemporaryDirectory() as d:
                        kwargs["tmp_path"] = pathlib.Path(d)
                        fn(**kwargs)
                else:
                    fn(**kwargs)
                print(f"PASS {path.name}::{name}")
            except Exception:
                failed += 1
                print(f"FAIL {path.name}::{name}")
                traceback.print_exc()
            finally:
                if monkeypatch:
                    monkeypatch.undo()
    print(f"{total - failed}/{total} tests passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
