import threading
from dataclasses import dataclass
from uuid import uuid4

import requests


@dataclass
class TestRunnerClient:
    build_id: str
    instance_id: str
    test_list: list[str]
    token: str = "SUPERSECRET"
    orchestrator_url: str = "http://localhost:5000"

    def run_tests(self) -> list[str]:
        """Run and return tests ran."""
        self.test_status = "ongoing"
        self.register_instance()

        test_ran = [test for test in self.run_test()]
        self.call_orchestrator("test-completed")
        return test_ran

    def register_instance(self):
        self.call_orchestrator("register-instance", data={"test_spec_list": self.test_list})

    def run_test(self):
        while self.test_status == "ongoing":
            next_test = self.get_next_test()
            if next_test:
                yield next_test

    def get_next_test(self):
        response_data = self.call_orchestrator("get-next-test-spec")
        self.test_status = response_data.get("status")
        return response_data.get("next_test")

    def call_orchestrator(self, endpoint, data=None):
        # Copied as is from frappe
        if data is None:
            data = {}
        headers = {
            "CI-BUILD-ID": self.build_id,
            "CI-INSTANCE-ID": self.instance_id,
            "REPO-TOKEN": self.token,
        }
        url = f"{self.orchestrator_url}/{endpoint}"
        res = requests.get(url, json=data, headers=headers)
        response_data = {}
        if "application/json" in res.headers.get("content-type"):
            response_data = res.json()

        return response_data


if __name__ == "__main__":
    build_id = str(uuid4())
    tests = [str(uuid4()) for _ in range(1000)]

    ran_tests = list()

    def _run_tests():
        orchestrator = TestRunnerClient(
            build_id=build_id, instance_id=str(uuid4()), test_list=tests
        )
        ran_in_one_thread = orchestrator.run_tests()
        assert len(ran_in_one_thread) >= 1  # Each client did "something"
        ran_tests.extend(ran_in_one_thread)

    threads = [threading.Thread(target=_run_tests) for _ in range(3)]
    for thread in threads:
        thread.start()
    for thread in threads:
        thread.join()

    assert set(tests) == set(ran_tests)  # Everything ran
    assert len(tests) == len(ran_tests)  # No duplicates
    print("All good")
