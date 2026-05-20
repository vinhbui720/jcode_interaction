#!/usr/bin/env python3
from __future__ import annotations

import importlib.util
import pathlib
import traceback

ROOT = pathlib.Path(__file__).resolve().parent
TEST_DIR = ROOT / "tests"


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
                if "tmp_path" in fn.__code__.co_varnames[: fn.__code__.co_argcount]:
                    import tempfile
                    with tempfile.TemporaryDirectory() as d:
                        fn(pathlib.Path(d))
                else:
                    fn()
                print(f"PASS {path.name}::{name}")
            except Exception:
                failed += 1
                print(f"FAIL {path.name}::{name}")
                traceback.print_exc()
    print(f"{total - failed}/{total} tests passed")
    return 1 if failed else 0


if __name__ == "__main__":
    raise SystemExit(main())
