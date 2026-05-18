# Skill: Add Hero Body

## Purpose
Add a new hero body (unit) to the AA2 data files. This involves researching the Dota2 hero's stats, translating them to the AA2 format, and creating the RON data file.

## Trigger
User asks to add a hero/body/unit to the game, e.g. "add Axe", "add Phantom Assassin as a hero body".

## Workflow

### 1. Research the Hero
Spawn `yolo-librarian` to look up the hero's stats from the Dota2 wiki:
- Primary attribute (Strength / Agility / Intelligence / Universal)
- Base stats: base_str, base_agi, base_int (at level 1)
- Stat gains: str_gain, agi_gain, int_gain (per level)
- Base attack time (BAT)
- Attack range (150 for melee, varies for ranged)
- Attack point (frontswing animation time)
- Move speed
- Turn rate
- Collision radius (typically 24 for most heroes, 27 for large)
- Is melee? (true/false)
- Base damage min/max (the BONUS portion only — NOT including primary attribute)
- Projectile speed (None for melee, Some(speed) for ranged)

**Important:** Base damage in Dota2 wiki includes the primary attribute. AA2 stores ONLY the bonus portion. Subtract the hero's base primary attribute from the wiki damage values.

### 2. Ask AA-Specific Questions
Before creating the file, ask the user:
- **Tier**: What tier is this hero? (D/C/B/A/S, stored as u8: 0=D, 1=C, 2=B, 3=A, 4=S)

Do NOT ask about Dota2 stats — those come from research. Only ask about AA2-specific design decisions.

### 3. Create the RON File
Write to `data/heroes/<snake_case_name>.ron` using this exact format:

```ron
HeroDef(
    name: "<Display Name>",
    primary_attribute: <Strength|Agility|Intelligence|Universal>,
    base_str: <f32>, base_agi: <f32>, base_int: <f32>,
    str_gain: <f32>, agi_gain: <f32>, int_gain: <f32>,
    base_attack_time: <f32>,
    attack_range: <f32>, attack_point: <f32>,
    move_speed: <f32>, turn_rate: <f32>, collision_radius: <f32>,
    tier: <u8>,
    is_melee: <bool>,
    base_damage_min: <f32>, base_damage_max: <f32>,
    projectile_speed: <None|Some(f32)>,
)
```

### 4. Verify
Run:
```bash
cargo test -p aa2-data 2>&1 | tail -5
cargo check -p aa2-game 2>&1 | tail -3
```

If there's a deserialization test for hero files, it should still pass.

## Tier Mapping
| Tier | u8 value | Meaning |
|------|----------|---------|
| D | 0 | Weakest bodies, available round 1 |
| C | 1 | Available round 3 |
| B | 2 | Available round 6 |
| A | 3 | Available round 9 |
| S | 4 | Available round 12 |

## Example
For Axe (melee STR hero, tier B):
```ron
HeroDef(
    name: "Axe",
    primary_attribute: Strength,
    base_str: 25.0, base_agi: 20.0, base_int: 18.0,
    str_gain: 3.4, agi_gain: 2.2, int_gain: 1.6,
    base_attack_time: 1.7,
    attack_range: 150.0, attack_point: 0.5,
    move_speed: 310.0, turn_rate: 0.6, collision_radius: 24.0,
    tier: 2,
    is_melee: true,
    base_damage_min: 27.0, base_damage_max: 31.0,
    projectile_speed: None,
)
```

## Notes
- All float values use one decimal place for consistency
- Turn rate in Dota2 is radians/0.03s; AA2 uses the same raw value
- Collision radius: 24 for most, 27 for large heroes (Sven, Doom, etc.)
- If the wiki shows different values for different patches, use the latest
