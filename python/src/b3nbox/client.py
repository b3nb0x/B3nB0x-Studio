import httpx


class B3nBoxError(Exception):
    pass


class B3nBoxClient:
    def __init__(
        self,
        base_url: str = "http://127.0.0.1:8300",
        api_key: str | None = None,
        token: str | None = None,
        timeout: float = 120.0,
    ):
        self.base_url = base_url.rstrip("/")
        headers = {}
        credential = token or api_key
        if credential:
            headers["Authorization"] = f"Bearer {credential}"
        self._client = httpx.Client(base_url=self.base_url, timeout=timeout, headers=headers)

    def login(self, api_key: str) -> dict:
        """Erzeugt eine Session aus einem API-Key."""
        resp = self._client.post("/v1/auth/login", json={"key": api_key})
        data = self._check(resp)
        if "token" in data:
            self._client.headers["Authorization"] = f"Bearer {data['token']}"
        return data

    def logout(self) -> dict:
        return self._post("/v1/auth/logout", {})

    def me(self) -> dict:
        return self._get("/v1/auth/me")

    def health(self) -> dict:
        return self._get("/health")

    def list_models(self) -> list[dict]:
        return self._get("/v1/models").get("models", [])

    def chat(self, messages: list[dict], route: str = "default", **params) -> dict:
        body = {"route": route, "messages": messages, **params}
        return self._post("/v1/chat", body)

    def get_routing(self) -> dict:
        return self._get("/v1/routing").get("routing", {})

    def set_route(self, key: str, backend: str, model: str) -> dict:
        return self._put(f"/v1/routing/{key}", {"backend": backend, "model": model})

    def _get(self, path: str) -> dict:
        resp = self._client.get(path)
        return self._check(resp)

    def _post(self, path: str, body: dict) -> dict:
        resp = self._client.post(path, json=body)
        return self._check(resp)

    def _put(self, path: str, body: dict) -> dict:
        resp = self._client.put(path, json=body)
        return self._check(resp)

    @staticmethod
    def _check(resp: httpx.Response) -> dict:
        if resp.status_code >= 400:
            raise B3nBoxError(f"API-Fehler {resp.status_code}: {resp.text}")
        return resp.json()
