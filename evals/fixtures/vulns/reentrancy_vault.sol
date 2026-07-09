// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @dev Eval fixture: classic reentrancy (state after call).
contract ReentrancyVault {
    mapping(address => uint256) public balances;

    function deposit() external payable {
        balances[msg.sender] += msg.value;
    }

    function withdraw() external {
        uint256 bal = balances[msg.sender];
        require(bal > 0, "zero");
        (bool ok,) = msg.sender.call{value: bal}("");
        require(ok, "send failed");
        balances[msg.sender] = 0;
    }
}
