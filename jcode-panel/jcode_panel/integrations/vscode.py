from __future__ import annotations

from pathlib import Path
import shutil

from .base import Integration, IntegrationStatus


class VSCodeIntegration(Integration):
    id = "vscode"
    name = "VS Code Context Extension"

    def __init__(self, project_root: Path):
        self.source_dir = project_root / "integrations" / "vscode_extension"
        self.target_dir = Path.home() / ".vscode" / "extensions" / "jcode-panel-context"

    def status(self) -> IntegrationStatus:
        installed = (self.target_dir / "package.json").exists()
        return IntegrationStatus(
            name=self.name,
            installed=installed,
            enabled=installed,
            message="Installed in ~/.vscode/extensions/jcode-panel-context. Reload VS Code to activate." if installed else "Not installed",
            install_hint="Install copies the extension locally. Reload VS Code, then move cursor in an editor before using @vscode.",
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
