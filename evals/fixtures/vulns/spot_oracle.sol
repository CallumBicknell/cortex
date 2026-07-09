// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

interface IPair {
    function getReserves() external view returns (uint112, uint112, uint32);
}

/// @dev Eval fixture: uses DEX spot reserves as price (flash-loanable).
contract SpotOracleConsumer {
    IPair public pair;

    constructor(address p) {
        pair = IPair(p);
    }

    function collateralValue(uint256 amountToken0) external view returns (uint256) {
        (uint112 r0, uint112 r1,) = pair.getReserves();
        // Spot price — manipulable in one transaction.
        return amountToken0 * uint256(r1) / uint256(r0);
    }
}
