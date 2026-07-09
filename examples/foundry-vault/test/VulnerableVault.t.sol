// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test} from "forge-std/Test.sol";
import {VulnerableVault} from "../src/VulnerableVault.sol";

contract VulnerableVaultTest is Test {
    VulnerableVault vault;

    function setUp() public {
        vault = new VulnerableVault();
    }

    function test_deposit_and_balance() public {
        vault.deposit{value: 1 ether}();
        assertEq(vault.balances(address(this)), 1 ether);
    }

    receive() external payable {}
}
