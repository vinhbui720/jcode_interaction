from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
import subprocess


@dataclass
class UpdateResult:
    ok: bool
    message: str
    changed: bool = False


def project_root() -> Path:
    return Path(__file__).resolve().parents[1]


def refresh_auto_integrations(root: Path) -> list[str]:
    """Refresh integrations that can be installed safely without prompting."""
    from .integrations import IntegrationRegistry

    messages: list[str] = []
    registry = IntegrationRegistry(root)
    for integration_id in ("vscode", "obsidian"):
        try:
            status = registry.install(integration_id)
            if status.installed:
                messages.append(f"{integration_id}: {status.message}")
            else:
                messages.append(f"{integration_id}: skipped ({status.message})")
        except Exception as exc:
            messages.append(f"{integration_id}: skipped ({exc})")
    return messages


def self_update(root: Path | None = None) -> UpdateResult:
    """Best-effort git self-update for source installs.

    Safe behavior: fetch first, fast-forward only, never force/reset.
    """
    root = root or project_root()
    if not (root.parent / ".git").exists() and not (root / ".git").exists():
        # package layout root is jcode-panel/, repo root is parent
        repo = root.parent
    else:
        repo = root if (root / ".git").exists() else root.parent
    try:
        before = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True, timeout=5).strip()
        # `--ff-only` is a pull/merge option, not a fetch option. Fetch is
        # non-destructive by default; the following pull enforces fast-forward
        # only so local work is never overwritten.
        subprocess.check_call(["git", "fetch", "origin"], cwd=repo, timeout=30)
        branch = subprocess.check_output(["git", "branch", "--show-current"], cwd=repo, text=True, timeout=5).strip() or "main"
        subprocess.check_call(["git", "pull", "--ff-only", "origin", branch], cwd=repo, timeout=60)
        after = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True, timeout=5).strip()
        update_message = "Already up to date" if before == after else f"Updated {before[:7]} → {after[:7]}"
        integration_messages = refresh_auto_integrations(root)
        if integration_messages:
            update_message += "\nIntegrations refreshed:\n" + "\n".join(f"- {message}" for message in integration_messages)
        return UpdateResult(True, update_message, before != after)
    except subprocess.CalledProcessError as exc:
        return UpdateResult(False, f"Update failed: {exc}. Resolve git state manually; no destructive action was taken.")
    except Exception as exc:
        return UpdateResult(False, f"Update unavailable: {exc}")
