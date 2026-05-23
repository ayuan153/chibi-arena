# Combat AI Specification

## Overview

Each unit's combat AI follows a strict **left-to-right priority system**. Every tick, the AI evaluates abilities in slot order and falls back to auto-attack. A unit does exactly one thing at a time: move, attack, or cast.

## Priority System

Every tick, for each unit that is Idle (not currently attacking, casting, or stunned):

```
for ability in equipped_abilities (left to right, slot 0 first):
    if ability.off_cooldown AND ability.has_mana AND valid_target_exists:
        match ability.cast_behavior:
            Seek | SeekPlus → MOVE toward target, CAST when in range
            Lazy → if target already in cast_range: CAST
                    else: skip (do not drive movement)
        if action taken: STOP evaluating further abilities

if no ability acted:
    AUTO-ATTACK (seek closest enemy, walk into attack range, attack)
```

## CastBehavior Types

| Behavior | Movement | Description |
|----------|----------|-------------|
| **Seek** | Yes — walks toward target | Actively pursues a valid target to cast on. Takes full movement priority. |
| **SeekPlus(range)** | Yes — walks toward target | Like Seek but with extended search range beyond cast_range. |
| **Lazy** | No — never drives movement | Only casts if a valid target is already within cast_range. Does not interrupt auto-attack pathing. |

## Auto-Attack (Implicit Rightmost Slot)

Auto-attack behaves like a permanent "Seek closest enemy" ability in a virtual rightmost slot:
- Seeks the closest living, non-invulnerable enemy
- Walks into attack range
- Attacks (frontswing → damage → backswing)
- Repeats

Auto-attack only activates when NO higher-priority ability is seeking or casting.

## Valid Targets

A "valid target" depends on the ability's damage type:
- **Magical/Pure abilities**: Target must NOT be spell-immune (magic immune)
- **Physical abilities**: Any living, non-invulnerable enemy
- **Auto-attack**: Any living, non-invulnerable enemy (ignores spell immunity)

## Key Behaviors

### Seek Walks Past Spell-Immune

If a Seek ability targets the closest non-spell-immune enemy, the unit will walk PAST any spell-immune frontliners to reach a valid target behind them. This is intentional and a core part of gameplay (e.g., a unit with Rage makes enemies walk past it).

### Lazy Does Not Drive Movement

A Lazy ability never causes the unit to move. If the unit is already walking toward an enemy for auto-attack and a Lazy target enters cast_range, the unit will cast it (interrupting the walk). But the Lazy ability itself never initiates movement.

### Slot Order Matters

Abilities in earlier slots (left) have higher priority. If slot 0 has a Seek ability off cooldown, it determines movement even if slot 1 also has a Seek ability with a closer target.

### No Kiting

Units cannot attack while moving. A unit is in exactly one state:
- **Idle** — evaluating what to do next
- **Moving** — walking toward a target (for seek ability or auto-attack)
- **Attacking** — in attack animation (frontswing → hit → backswing)
- **Casting** — in cast animation (cast point → effect)
- **Stunned** — cannot act

## State Transitions

```
Idle → evaluate priorities → Moving (toward target) or Casting (if in range)
Moving → arrive in range → Casting or Attacking
Attacking → backswing complete → Idle (re-evaluate)
Casting → cast complete → Idle (re-evaluate)
Stunned → stun expires → Idle (re-evaluate)
```

## Examples

### Example 1: Burrowstrike (Lazy) + Magic Missile (Seek)

Unit has: [Burrowstrike (Lazy, 300 range), Magic Missile (Seek, 600 range)]

1. Tick 0: Check Burrowstrike — off CD, Lazy, no target in 300 range → skip
2. Check Magic Missile — off CD, Seek, target at 800 → MOVE toward target
3. Unit walks toward target...
4. Target enters 600 range → CAST Magic Missile
5. After cast: re-evaluate. Burrowstrike still Lazy, no target in 300 → skip. Magic Missile on CD → skip. Auto-attack → walk toward closest enemy, attack.
6. During auto-attack fight, enemy enters 300 range → next Idle tick, Burrowstrike fires (it's slot 0, higher priority than auto-attack)

### Example 2: Seek ability walks past spell-immune

Unit has: [Spirit Lance (Seek, Magical, 600 range)]

Enemy team: [Frontliner with Rage (spell-immune), Backliner at 1200 range]

1. Check Spirit Lance — Seek, valid target = Backliner (frontliner is spell-immune)
2. Unit walks PAST the spell-immune frontliner toward the backliner
3. Backliner enters 600 range → CAST Spirit Lance
4. After cast: auto-attack takes over → attacks closest enemy (the frontliner, since auto-attack ignores spell immunity)
