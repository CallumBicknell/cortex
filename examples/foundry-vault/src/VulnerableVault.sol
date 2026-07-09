// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @notice Intentionally vulnerable vault for Cortex audit demos / evals.
/// @dev Do not deploy. Classic reentrancy: state updated after external call.
contract VulnerableVault {
    mapping(address => uint256) public balances;

    function deposit() external payable {
        balances[msg.sender] += msg.value;
    }

    /// @dev VULNERABLE — CEI violation (classic reentrancy).
    function withdraw() external {
        uint256 bal = balances[msg.sender];
        require(bal > 0, "zero");
        (bool ok,) = msg.sender.call{value: bal}("");
        require(ok, "send failed");
        balances[msg.sender] = 0;
    }

    receive() external payable {}
}
