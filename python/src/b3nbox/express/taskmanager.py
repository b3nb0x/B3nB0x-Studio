import uuid

from ..client import B3nBoxClient


class TaskManager:
    def __init__(self, client: B3nBoxClient):
        self.client = client

    def list(self) -> list[dict]:
        return self.client._get("/v1/tasks").get("tasks", [])

    def create(self, name: str, prompt: str, route: str = "default", script: str | None = None) -> dict:
        task = {
            "id": f"task-{uuid.uuid4().hex[:8]}",
            "name": name,
            "prompt": prompt,
            "route": route,
        }
        if script:
            task["script"] = script
        return self.client._post("/v1/tasks", task)

    def run(self, task_id: str) -> dict:
        return self.client._post(f"/v1/tasks/{task_id}/run", {})
