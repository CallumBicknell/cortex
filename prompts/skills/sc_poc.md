# Exploit PoC scaffolding (Foundry)

Turn a confirmed or high-confidence vulnerability into a **reproducing forge test**.

## When

- User asks for a PoC, exploit test, or proof
- After a FINDING with clear root cause (especially Critical/High)
- Prefer PoC over prose alone when the environment is Foundry

## Layout

```text
test/exploit/<ShortName>.t.sol
```

Exclude production deployment of exploit contracts. Keep demos under `test/`.

## Template pattern

```solidity
// SPDX-License-Identifier: MIT
pragma solidity ^0.8.24;

import {Test, console2} from "forge-std/Test.sol";
// import {Target} from "../../src/Target.sol";

/// @notice PoC for: <one-line bug title>
/// @dev Expected: test proves fund loss / invariant break / unauthorized action.
contract Exploit_<ShortName>_Test is Test {
    // Target target;
    // address attacker = makeAddr("attacker");

    function setUp() public {
        // target = new Target(...);
        // deal / deal tokens
    }

    function test_poc_<slug>() public {
        // 1. Arrange: balances before
        // 2. Act: attack steps (prank, callbacks, flash loan sketch)
        // 3. Assert: attacker profit OR invariant broken OR auth bypass
        // assertGt(attackerProfit, 0);
    }
}
```

Also see `examples/foundry-vault/test/exploit/ReentrancyPoC.t.sol` for a full sketch.

## Rules

1. **Minimal** — smallest steps that demonstrate impact.
2. **Deterministic** — no flaky timestamps unless essential (`vm.warp` fixed).
3. **Named** — test name references the bug class.
4. **Honest** — if you only sketch the attack without a green/red assertion, label it a **sketch**, not a proven PoC.
5. Run `forge test --match-path test/exploit/* -vvv` when forge-std is available.
6. Never use mainnet keys or real value.

## Report link

In the audit finding, set:

- **Proof:** `test/exploit/….t.sol::test_poc_…` (or “sketch only”)
