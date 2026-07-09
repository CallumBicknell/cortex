// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

/// @dev Eval fixture: naive share math without virtual offset (inflation risk).
contract NaiveVault {
    mapping(address => uint256) public shares;
    uint256 public totalShares;
    uint256 public totalAssets;

    function deposit(uint256 assets) external returns (uint256 minted) {
        if (totalShares == 0) {
            minted = assets;
        } else {
            minted = assets * totalShares / totalAssets;
        }
        shares[msg.sender] += minted;
        totalShares += minted;
        totalAssets += assets;
    }
}
