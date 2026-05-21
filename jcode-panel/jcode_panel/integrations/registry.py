from __future__ import annotations

from pathlib import Path

from .base import IntegrationStatus
from .browser import BrowserExtensionIntegration
from .obsidian import ObsidianIntegration
from .vscode import VSCodeIntegration


class IntegrationRegistry:
    def __init__(self, project_root: Path, obsidian_vault: Path | None = None):
        self.integrations = {
            "browser": BrowserExtensionIntegration(project_root),
            "vscode": VSCodeIntegration(project_root),
            "obsidian": ObsidianIntegration(project_root, obsidian_vault),
        }

    def list_statuses(self) -> list[IntegrationStatus]:
        return [integration.status() for integration in self.integrations.values()]

    def install(self, integration_id: str) -> IntegrationStatus:
        return self.integrations[integration_id].install()

    def uninstall(self, integration_id: str) -> IntegrationStatus:
        return self.integrations[integration_id].uninstall()
