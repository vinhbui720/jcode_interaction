from __future__ import annotations

from pathlib import Path
import shutil
import subprocess
import tempfile

from .base import Integration, IntegrationStatus


class VSCodeIntegration(Integration):
    id = "vscode"
    name = "VS Code Context Extension"

    def __init__(self, project_root: Path):
        self.source_dir = project_root / "integrations" / "vscode_extension"
        self.target_dir = Path.home() / ".vscode" / "extensions" / "jcode-panel-context"

    def status(self) -> IntegrationStatus:
        installed = self._code_extension_installed() or (self.target_dir / "package.json").exists()
        return IntegrationStatus(
            name=self.name,
            installed=installed,
            enabled=installed,
            message="Installed as jcode-panel.jcode-panel-context. Reload VS Code to activate." if installed else "Not installed",
            install_hint="Install uses VS Code CLI when available, with a local folder fallback. Reload VS Code, then move cursor in an editor before using @vscode.",
        )

    def install(self) -> IntegrationStatus:
        self.target_dir.parent.mkdir(parents=True, exist_ok=True)
        if self.target_dir.exists():
            shutil.rmtree(self.target_dir)
        shutil.copytree(self.source_dir, self.target_dir)
        self._install_with_code_cli()
        return self.status()

    def uninstall(self) -> IntegrationStatus:
        if self.target_dir.exists():
            shutil.rmtree(self.target_dir)
        return self.status()

    def _code_extension_installed(self) -> bool:
        code = shutil.which("code")
        if not code:
            return False
        try:
            result = subprocess.run([code, "--list-extensions"], stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True, timeout=20)
            return "jcode-panel.jcode-panel-context" in result.stdout.splitlines()
        except Exception:
            return False

    def _install_with_code_cli(self) -> None:
        code = shutil.which("code")
        npx = shutil.which("npx")
        if not code or not npx:
            return
        try:
            with tempfile.TemporaryDirectory() as tmp:
                vsix = Path(tmp) / "jcode-panel-context.vsix"
                subprocess.run(
                    [npx, "--yes", "@vscode/vsce", "package", "--allow-missing-repository", "--out", str(vsix)],
                    cwd=str(self.source_dir),
                    check=True,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                    timeout=120,
                )
                subprocess.run([code, "--install-extension", str(vsix), "--force"], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=60)
        except Exception:
            return
