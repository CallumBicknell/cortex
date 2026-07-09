// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @dev Eval fixture: initializer not protected / can be called by anyone.
contract OpenInit {
    address public owner;
    bool public ready;

    function initialize(address newOwner) external {
        owner = newOwner;
        ready = true;
    }

    function sweep(address to) external {
        require(msg.sender == owner, "not owner");
        payable(to).transfer(address(this).balance);
    }

    receive() external payable {}
}
