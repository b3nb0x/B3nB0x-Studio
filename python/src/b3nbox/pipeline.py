from enum import Enum

from .client import B3nBoxClient


class Phase(str, Enum):
    ANALYSE = "analyse"
    PLAN = "plan"
    IMPLEMENT = "implement"
    CONTROL = "control"


class PipelineClient:
    def __init__(self, client: B3nBoxClient):
        self.client = client

    def run(self, task: str) -> dict:
        return self.client._post("/v1/pipeline/runs", {"task": task})

    def list_runs(self) -> list[dict]:
        return self.client._get("/v1/pipeline/runs").get("runs", [])

    def get_run(self, run_id: str) -> dict:
        return self.client._get(f"/v1/pipeline/runs/{run_id}")

    def switch_phase(self, phase: Phase, backend: str, model: str) -> dict:
        return self.client._post(
            "/v1/pipeline/switch",
            {"phase": phase.value, "backend": backend, "model": model},
        )
