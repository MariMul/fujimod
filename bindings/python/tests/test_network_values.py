"""Radio/NAND values work in native production wheels too."""

from network_contract import check_network_values


def test_network_values():
    assert check_network_values()
