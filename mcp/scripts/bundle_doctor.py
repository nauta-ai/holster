from __future__ import annotations

import shutil
import stat
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SOURCE_CANDIDATES = [
    ROOT / "target" / "release" / "holster-doctor",
    ROOT / "target" / "x86_64-unknown-linux-gnu" / "release" / "holster-doctor",
    ROOT / "target" / "aarch64-apple-darwin" / "release" / "holster-doctor",
]
DEST = ROOT / "mcp" / "src" / "holster_mcp" / "bin" / "holster-doctor"


def main() -> None:
    for source in SOURCE_CANDIDATES:
        if source.is_file():
            DEST.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, DEST)
            mode = DEST.stat().st_mode
            DEST.chmod(mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
            print(f"bundled {source} -> {DEST}")
            return
    candidates = "\n".join(str(path) for path in SOURCE_CANDIDATES)
    raise SystemExit(f"holster-doctor binary not found; checked:\n{candidates}")


if __name__ == "__main__":
    main()
