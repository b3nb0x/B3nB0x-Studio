import subprocess

from ..client import B3nBoxClient


class ScriptRunner:
    def __init__(self, client: B3nBoxClient, allowed_dirs: list[str] | None = None):
        self.client = client
        self.allowed_dirs = allowed_dirs or ["scripts"]
        self.scripts: dict[str, str] = {}

    def register(self, name: str, path: str) -> None:
        self.scripts[name] = path

    def list_scripts(self) -> list[str]:
        return sorted(self.scripts.keys())

    def run(self, name: str, args: list[str] | None = None, approval: bool = False) -> dict:
        path = self.scripts.get(name)
        if path is None:
            return {"status": "error", "error": f"Skript '{name}' nicht registriert"}
        if not any(path.startswith(d) for d in self.allowed_dirs):
            return {"status": "blocked", "error": f"Skript liegt außerhalb der erlaubten Verzeichnisse: {path}"}
        if not approval:
            return {"status": "awaiting-approval", "script": path, "args": args or [], "note": "Erneut mit approval=True aufrufen"}
        result = subprocess.run([path, *(args or [])], capture_output=True, text=True)
        return {
            "status": "completed",
            "returncode": result.returncode,
            "stdout": result.stdout,
            "stderr": result.stderr,
        }
