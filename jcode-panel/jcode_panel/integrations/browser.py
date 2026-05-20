from __future__ import annotations

from pathlib import Path
import shutil

from .base import Integration, IntegrationStatus


class BrowserExtensionIntegration(Integration):
    id = "browser"
    name = "Browser Context Extension"

    def __init__(self, project_root: Path):
        self.source_dir = project_root / "extension"
        self.target_dir = Path.home() / ".local" / "share" / "jcode-panel" / "integrations" / "browser_extension"

    def status(self) -> IntegrationStatus:
        installed = (self.target_dir / "manifest.json").exists()
        return IntegrationStatus(
            name=self.name,
            installed=installed,
            enabled=installed,
            message="Installed locally; load it from browser extension developer mode." if installed else "Not installed",
            install_hint="Install copies files locally. Browser still requires manual load/enable for safety.",
        )

    def install(self) -> IntegrationStatus:
        self.target_dir.parent.mkdir(parents=True, exist_ok=True)
        if self.target_dir.exists():
            shutil.rmtree(self.target_dir)
        shutil.copytree(self.source_dir, self.target_dir)
        return self.status()

    def uninstall(self) -> IntegrationStatus:
        if self.target_dir.exists():
            shutil.rmtree(self.target_dir)
        return self.status()
