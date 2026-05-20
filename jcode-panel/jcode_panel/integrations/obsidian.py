from __future__ import annotations

from pathlib import Path
import shutil

from .base import Integration, IntegrationStatus


class ObsidianIntegration(Integration):
    id = "obsidian"
    name = "Obsidian Context Plugin"

    def __init__(self, project_root: Path, vault_path: Path | None = None):
        self.source_dir = project_root / "integrations" / "obsidian_plugin"
        self.vault_path = vault_path

    def _target_dir(self) -> Path | None:
        if not self.vault_path:
            return None
        return self.vault_path / ".obsidian" / "plugins" / "jcode-panel"

    def status(self) -> IntegrationStatus:
        target = self._target_dir()
        installed = bool(target and (target / "manifest.json").exists())
        return IntegrationStatus(
            name=self.name,
            installed=installed,
            enabled=installed,
            message="Installed in configured vault" if installed else "Vault not configured or plugin not installed",
            install_hint="Set an Obsidian vault path, then install. This scaffold is ready for later richer context capture.",
        )

    def install(self) -> IntegrationStatus:
        target = self._target_dir()
        if not target:
            return self.status()
        target.parent.mkdir(parents=True, exist_ok=True)
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(self.source_dir, target)
        return self.status()

    def uninstall(self) -> IntegrationStatus:
        target = self._target_dir()
        if target and target.exists():
            shutil.rmtree(target)
        return self.status()
