from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path


@dataclass
class IntegrationStatus:
    name: str
    installed: bool
    enabled: bool
    message: str
    install_hint: str = ""


class Integration:
    """Installable app integration contract.

    Each integration owns its files and installer logic so future apps like
    Obsidian can be added without changing the browser bridge or core panel.
    """

    id: str = "base"
    name: str = "Base"
    source_dir: Path

    def status(self) -> IntegrationStatus:
        raise NotImplementedError

    def install(self) -> IntegrationStatus:
        raise NotImplementedError

    def uninstall(self) -> IntegrationStatus:
        raise NotImplementedError
