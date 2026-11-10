import subprocess

from ..client import B3nBoxClient
from .taskmanager import TaskManager


class RebootScheduler:
    """Plant Reboot-Scheduler als Express-Task.
    Reboots werden nur mit expliziter Freigabe (approval) ausgeführt.
    """

    def __init__(self, client: B3nBoxClient, auto_approve: bool = False):
        self.client = client
        self.tasks = TaskManager(client)
        self.auto_approve = auto_approve

    def register_reboot(self, at: str, reason: str = "Geplanter Wartungs-Reboot") -> dict:
        return self.tasks.create(
            name=f"Reboot um {at}",
            prompt=f"Bestätige den geplanten Reboot um {at}. Grund: {reason}",
            route="scheduler",
        )

    def execute_reboot(self, dry_run: bool = True) -> dict:
        if not self.auto_approve and not dry_run:
            return {"status": "blocked", "error": "Reboot benötigt explizite Freigabe (auto_approve=True oder dry_run=True)"}
        if dry_run:
            return {"status": "dry-run", "command": "sudo systemctl reboot", "note": "Kein echter Reboot ausgeführt"}
        result = subprocess.run(["sudo", "systemctl", "reboot"], capture_output=True, text=True)
        return {"status": "completed", "returncode": result.returncode, "stderr": result.stderr}
