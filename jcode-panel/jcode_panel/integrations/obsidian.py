from __future__ import annotations

from pathlib import Path
import json
import shutil

from .base import Integration, IntegrationStatus


class ObsidianIntegration(Integration):
    id = "obsidian"
    name = "Obsidian Context Plugin"

    def __init__(self, project_root: Path, vault_path: Path | None = None):
        self.source_dir = project_root / "integrations" / "obsidian_plugin"
        self.vault_path = vault_path or self._detect_vault_path()

    def _detect_vault_path(self) -> Path | None:
        config = Path.home() / ".config" / "obsidian" / "obsidian.json"
        try:
            data = json.loads(config.read_text())
            vaults = data.get("vaults") or {}
            ordered = sorted(vaults.values(), key=lambda item: (not bool(item.get("open")), -int(item.get("ts") or 0)))
            for item in ordered:
                path = Path(str(item.get("path") or "")).expanduser()
                if path.exists():
                    return path
        except Exception:
            return None
        return None

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
            install_hint="Auto-detects the latest/open Obsidian vault from ~/.config/obsidian/obsidian.json. Enable the community plugin in Obsidian after install.",
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
