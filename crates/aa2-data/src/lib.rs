use serde::{Deserialize, Serialize};

// ─── Buff value types (canonical source; re-exported by aa2-sim::buff) ───

/// Behavior when the same buff is reapplied.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum StackBehavior {
    /// Same source reapplies → timer resets (default).
    RefreshDuration,
    /// Multiple instances accumulate up to max stacks.
    StackIntensity(u32),
    /// Each application is tracked separately.
    Independent,
}

/// Determines what strength of dispel can remove this buff.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DispelType {
    /// Removed by any dispel.
    BasicDispel,
    /// Only removed by strong dispel.
    StrongDispel,
    /// Cannot be removed.
    Undispellable,
}

/// Status effect flags applied by buffs/debuffs.
#[derive(Serialize, Deserialize, Debug, Default, Clone, Copy, PartialEq)]
pub struct StatusFlags {
    /// Prevents all actions.
    pub stunned: bool,
    /// Prevents ability casting.
    pub silenced: bool,
    /// Prevents attacking.
    pub disarmed: bool,
    /// Prevents movement.
    pub rooted: bool,
    /// Prevents all actions + sets MS to 140 + disables passives.
    pub hexed: bool,
    /// Cannot be targeted or take damage.
    pub invulnerable: bool,
    /// Immune to magic damage, most debuffs, and spell targeting.
    pub magic_immune: bool,
}

impl StatusFlags {
    /// Merge multiple status flags by OR-ing all fields together.
    pub fn merge(flags: &[StatusFlags]) -> StatusFlags {
        let mut result = StatusFlags::default();
        for f in flags {
            result.stunned |= f.stunned;
            result.silenced |= f.silenced;
            result.disarmed |= f.disarmed;
            result.rooted |= f.rooted;
            result.hexed |= f.hexed;
            result.invulnerable |= f.invulnerable;
            result.magic_immune |= f.magic_immune;
        }
        result
    }
}

/// Additive stat modifiers from buffs.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub struct StatModifier {
    /// Bonus armor (additive).
    #[serde(default)]
    pub bonus_armor: f32,
    /// Bonus attack speed (additive).
    #[serde(default)]
    pub bonus_attack_speed: f32,
    /// Bonus move speed (additive).
    #[serde(default)]
    pub bonus_move_speed: f32,
    /// Bonus damage (additive).
    #[serde(default)]
    pub bonus_damage: f32,
    /// Bonus magic resistance (multiplicative with base).
    #[serde(default)]
    pub bonus_magic_resistance: f32,
    /// Bonus HP regen per second (additive).
    #[serde(default)]
    pub bonus_hp_regen: f32,
    /// Bonus strength (adds HP + damage if STR primary).
    #[serde(default)]
    pub bonus_strength: f32,
    /// Bonus agility (adds armor, AS, damage if AGI primary).
    #[serde(default)]
    pub bonus_agi: f32,
    /// Bonus intelligence (adds mana, mana regen, damage if INT primary).
    #[serde(default)]
    pub bonus_int: f32,
    /// Status resistance (0.5 = 50% shorter debuffs).
    #[serde(default)]
    pub status_resistance: f32,
}

// ─── Composable effect schema (data-only; resolvers live in aa2-sim) ───

/// Data definition for a periodic tick effect (DoT or HoT).
/// The runtime `TickEffect` in aa2-sim adds `ticks_until_next` state.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct TickEffectDef {
    /// Positive = damage, negative = heal.
    pub damage: f32,
    /// Type of damage dealt.
    pub damage_type: DamageType,
    /// Apply every N ticks.
    pub interval_ticks: u32,
}

/// Data definition for a buff/debuff, parameterized per-level where applicable.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BuffDef {
    /// Name identifier for this buff.
    pub name: String,
    /// Duration in seconds per ability level.
    pub duration: Vec<f32>,
    /// Status effects this buff applies.
    pub status: StatusFlags,
    /// Stat modifiers this buff applies.
    #[serde(default)]
    pub stat_modifier: Option<StatModifier>,
    /// Periodic tick effect (DoT/HoT).
    #[serde(default)]
    pub tick_effect: Option<TickEffectDef>,
    /// How this buff stacks with itself.
    pub stacking: StackBehavior,
    /// What dispel strength removes this buff.
    pub dispel_type: DispelType,
    /// true = negative effect (from enemy).
    #[serde(default)]
    pub is_debuff: bool,
    /// If true, this debuff applies even to magic immune units.
    #[serde(default)]
    pub pierces_magic_immunity: bool,
    /// Percentage of autoattack damage reflected back to attacker (0.0 to 1.0).
    #[serde(default)]
    pub damage_reflection_pct: f32,
}

/// When an effect fires.
/// More variants (OnAttack, OnHit, OnKill, Periodic) added as abilities are ported.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Trigger {
    /// Fires when the ability is cast.
    OnCast,
}

/// Who/where the effect targets.
/// Minimal set for the proof; will grow to cover single-target, ally, point.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum TargetingSpec {
    /// Targets the caster.
    Caster,
    /// Targets enemies within the delivery area.
    EnemiesInDelivery,
}

/// How the effect reaches affected units.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Delivery {
    /// Applied immediately with no travel time.
    Instant,
    /// Expands outward from origin at a fixed speed.
    ExpandingWave {
        /// Maximum radius per ability level.
        max_radius: Vec<f32>,
        /// Expansion speed (units/sec).
        speed: f32,
    },
}

/// What happens to each affected unit.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Payload {
    /// Deal damage of the given type.
    Damage {
        /// Damage type.
        kind: DamageType,
        /// Base damage per ability level.
        base: Vec<f32>,
    },
    /// Apply a buff/debuff.
    ApplyBuff(BuffDef),
    /// Dispel debuffs up to the given strength.
    Dispel {
        /// Maximum dispel strength to remove.
        strength: DispelType,
    },
    /// Trigger a chained sub-effect. Recursion is bounded by
    /// `MAX_EFFECT_CHAIN_DEPTH` (enforced in aa2-sim, added later).
    Chain(Box<EffectSpec>),
}

/// A composable effect specification: trigger + targeting + delivery + payloads.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct EffectSpec {
    /// When this effect fires.
    pub trigger: Trigger,
    /// Who/where it targets.
    pub targeting: TargetingSpec,
    /// How it reaches targets.
    pub delivery: Delivery,
    /// What happens to each affected unit.
    pub payload: Vec<Payload>,
}

/// How an effect interacts with illusions.
/// Determines whether illusions can use/benefit from this effect.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
pub enum IllusionInteraction {
    /// Works fully on illusions (crits, lifesteal, mana break, curse of avernus)
    Full,
    /// Does NOT work on illusions (fury swipes, essence shift, glaives, bash, cleave)
    #[default]
    Disabled,
    /// Illusion carries this as an aura to nearby allies (radiance, inner beast)
    CarriesAura,
}

/// How the AI handles targeting for this ability.
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub enum CastBehavior {
    /// Won't walk into range. Only casts if a valid target is already in range.
    Lazy,
    /// Walks toward closest valid target until in cast range, then casts.
    #[default]
    Seek,
    /// Walks toward closest valid target within cast_range + extra units.
    SeekPlus(f32),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Attribute {
    Strength,
    Agility,
    Intelligence,
    Universal,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum DamageType {
    Physical,
    Magical,
    Pure,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum TargetType {
    SingleEnemy,
    SingleAlly,
    SingleAllyHG,
    PointAoE,
    NoTarget,
    Passive,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum AoeShape {
    Circle { radius: f32 },
    Cone { angle: f32, range: f32 },
    Line { width: f32, length: f32 },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Effect {
    Damage { kind: DamageType, base: Vec<f32> },
    ApplyBuff { name: String, duration: f32 },
    Heal { base: Vec<f32> },
    Summon { unit: String, count: u32 },
    /// Dark Pact style: delayed pulsing AoE around self with self-damage and per-pulse dispel.
    /// Caster can act freely during delay. Each pulse independently dispels.
    DarkPact {
        kind: DamageType,
        total_damage: Vec<f32>,
        radius: Vec<f32>,
        self_damage_pct: f32,
        delay: f32,
        pulse_count: u32,
        pulse_interval: f32,
        dispel_self: bool,
        non_lethal: bool,
    },
    /// Buff applied to target AND caster (Heavenly Grace self-cast mechanic).
    BuffTargetAndSelf {
        name: String,
        duration: Vec<f32>,
        hp_regen: Vec<f32>,
        strength: Vec<f32>,
        status_resistance: Vec<f32>,
        #[serde(default)]
        dispel_on_cast: bool,
    },
    /// Expanding wave AoE stun (Ravage). Hits units when wave reaches them.
    ExpandingWaveStun {
        damage: Vec<f32>,
        stun_duration: Vec<f32>,
        radius: Vec<f32>,
        wave_speed: f32,
    },
    /// Fury Swipes: per-target stacking flat damage, added post-crit.
    FurySwipes {
        damage_per_stack: Vec<f32>,
        stack_duration: Vec<f32>,
        armor_reduction_per_stack: Vec<f32>,
    },
    /// Chaos Strike: PRD-based crit with lifesteal.
    ChaosStrike {
        proc_chance: Vec<f32>,
        crit_min: Vec<f32>,
        crit_max: Vec<f32>,
        lifesteal: Vec<f32>,
    },
    /// Essence Shift: steal stats on attack.
    EssenceShift {
        str_steal: Vec<f32>,
        agi_steal: Vec<f32>,
        int_steal: Vec<f32>,
        agi_gain: Vec<f32>,
        duration: Vec<f32>,
    },
    /// Glaives of Wisdom: mana-cost attack modifier dealing bonus magical damage based on INT.
    /// Does not pierce debuff immunity. Super steals INT on kill. Gaben bounces.
    GlaivesOfWisdom {
        int_damage_factor: Vec<f32>,
        mana_cost: Vec<f32>,
        int_steal_per_attack: Vec<f32>,  // INT stolen per hit (2/3/5)
        steal_duration: Vec<f32>,         // duration of temp steal in seconds (10/20/40)
        steal_int_on_kill: Vec<f32>,      // permanent INT on kill (0 base, 1 at Super)
        steal_radius: f32,
        bounce_radius: Vec<f32>,
    },
    /// Burrowstrike: line AoE stun + damage, caster travels at speed to end point.
    /// During travel, caster is invulnerable and untargetable.
    Burrowstrike {
        damage: Vec<f32>,
        stun_duration: Vec<f32>,
        range: Vec<f32>,
        width: f32,
        travel_speed: f32,              // units/sec (2000)
        caustic_finale_damage: Vec<f32>, // on-death explosion damage (0 = none, >0 at Super+)
        caustic_finale_radius: f32,      // explosion radius (400)
    },
    /// Rage: self-buff granting magic immunity + basic dispel on cast.
    Rage {
        duration: Vec<f32>,
    },
    /// Spirit Lance: projectile that damages, slows, and spawns an illusion at target.
    SpiritLance {
        damage: Vec<f32>,
        slow_pct: Vec<f32>,
        slow_duration: Vec<f32>,
        projectile_speed: f32,
        illusion_damage_dealt: Vec<f32>,
        illusion_damage_taken: f32,
        illusion_duration: Vec<f32>,
        bounce_radius: Vec<f32>,
        bounce_count: Vec<u32>,
    },
    /// Spear of Mars: projectile that impales first hero, drags to wall, damages pass-through.
    SpearOfMars {
        damage: Vec<f32>,
        stun_duration: Vec<f32>,
        range: Vec<f32>,
        travel_speed: f32,
        width: f32,
        fire_trail_dps: Vec<f32>,
        fire_trail_slow: Vec<f32>,
        fire_trail_duration: Vec<f32>,
        wall_bounces: Vec<u32>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeroDef {
    pub name: String,
    pub primary_attribute: Attribute,
    pub base_str: f32,
    pub base_agi: f32,
    pub base_int: f32,
    pub str_gain: f32,
    pub agi_gain: f32,
    pub int_gain: f32,
    pub base_attack_time: f32,
    pub attack_range: f32,
    pub attack_point: f32,
    pub move_speed: f32,
    pub turn_rate: f32,
    pub collision_radius: f32,
    pub tier: u8,
    pub is_melee: bool,
    /// Raw base damage range (before primary attribute bonus). [min, max]
    /// Each attack rolls uniformly between min and max (inclusive).
    pub base_damage_min: f32,
    pub base_damage_max: f32,
    /// Projectile speed for ranged heroes (units/sec). Ignored for melee.
    pub projectile_speed: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AbilityDef {
    pub name: String,
    pub cooldown: Vec<f32>,
    pub mana_cost: Vec<f32>,
    pub cast_point: f32,
    pub targeting: TargetType,
    pub effects: Vec<Effect>,
    pub description: String,
    #[serde(default)]
    pub is_ultimate: bool,
    /// Shape of the AoE for `PointAoE` abilities. `None` for non-AoE abilities.
    #[serde(default)]
    pub aoe_shape: Option<AoeShape>,
    /// Maximum range at which this ability can be cast.
    #[serde(default = "default_cast_range")]
    pub cast_range: f32,
    /// How the AI handles targeting for this ability.
    #[serde(default)]
    pub cast_behavior: CastBehavior,
    /// If set, ability uses a charge system instead of normal cooldown.
    #[serde(default)]
    pub max_charges: Option<u32>,
    /// If Some, this ability uses the composable resolver and the legacy
    /// `effects` Vec is ignored during execution.
    #[serde(default)]
    pub effect_specs: Option<Vec<EffectSpec>>,
}

/// Default cast range for abilities (600 units).
fn default_cast_range() -> f32 {
    600.0
}

/// Look up a per-level value from a Vec (1-indexed: level 1 = index 0).
/// Clamps to the last element if level exceeds the array length.
pub fn value_at_level(values: &[f32], level: u8) -> f32 {
    let idx = (level.saturating_sub(1) as usize).min(values.len().saturating_sub(1));
    values[idx]
}



#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct StatBonuses {
    pub strength: f32,
    pub agility: f32,
    pub intelligence: f32,
    pub attack_speed: f32,
    pub move_speed: f32,
    pub armor: f32,
    pub magic_resistance: f32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ItemDef {
    pub name: String,
    pub tier: u8,
    pub effects: Vec<Effect>,
    pub stat_bonuses: StatBonuses,
}

/// God passive types.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GodPassive {
    /// Archmage: chance to +1 level a random ability at shop start.
    Sorcery {
        /// Chance to trigger at shop phase start (0.0 to 1.0).
        trigger_chance: f32,
    },
    /// Paladin: Buff selected unit with bonus HP and damage reflection.
    RadiantShield {
        /// Bonus HP = multiplier * round_number.
        hp_per_round: f32,
        /// Damage reflection percentage (0.0 to 1.0).
        reflection_pct: f32,
    },
}

/// God definition for the game.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct God {
    /// Display name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Passive effect.
    pub passive: GodPassive,
}

/// Loads a single `God` from a `.ron` file at the given path.
pub fn load_god_def(path: &std::path::Path) -> Result<God, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Loads all `God`s from `.ron` files in the given directory.
pub fn load_all_gods(dir: &std::path::Path) -> Result<Vec<God>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    let mut gods = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{dir:?}: {e}"))?.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            gods.push(load_god_def(&path)?);
        }
    }
    gods.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(gods)
}

/// A fully-configured unit ready for combat.
/// Bridge between game systems (draft) and simulation.
#[derive(Debug, Clone)]
pub struct UnitConfig {
    /// The hero definition for this unit.
    pub hero: HeroDef,
    /// Equipped abilities with their levels.
    pub abilities: Vec<(AbilityDef, u8)>,
    /// Number of ability slots available.
    pub slot_count: u8,
    /// Hero level (1-30). Scales base attributes via gain per level.
    pub level: u8,
}

impl UnitConfig {
    /// Create a new UnitConfig with just a hero and no abilities.
    pub fn new(hero: HeroDef) -> Self {
        Self { hero, abilities: Vec::new(), slot_count: 4, level: 1 }
    }

    /// Add an ability at the given level.
    pub fn with_ability(mut self, ability: AbilityDef, level: u8) -> Self {
        self.abilities.push((ability, level));
        self
    }

    /// Set the hero level.
    pub fn with_level(mut self, level: u8) -> Self {
        self.level = level;
        self
    }
}

/// A loadout file specifying a hero + equipped abilities for dev/testing.
/// Ability and hero names are resolved to file paths at load time.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Loadout {
    /// Hero name (resolved to data/heroes/{name}.ron).
    pub hero: String,
    /// Ability names with levels (resolved to data/abilities/{name}.ron).
    pub abilities: Vec<(String, u8)>,
}

/// Loads a single `HeroDef` from a `.ron` file at the given path.
pub fn load_hero_def(path: &std::path::Path) -> Result<HeroDef, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Loads a single `AbilityDef` from a `.ron` file at the given path.
pub fn load_ability_def(path: &std::path::Path) -> Result<AbilityDef, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Loads all `HeroDef`s from `.ron` files in the given directory.
pub fn load_all_heroes(dir: &std::path::Path) -> Result<Vec<HeroDef>, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{dir:?}: {e}"))?;
    let mut heroes = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| format!("{dir:?}: {e}"))?.path();
        if path.extension().is_some_and(|ext| ext == "ron") {
            heroes.push(load_hero_def(&path)?);
        }
    }
    Ok(heroes)
}

/// Load a `Loadout` from a `.ron` file.
pub fn load_loadout(path: &std::path::Path) -> Result<Loadout, String> {
    let contents = std::fs::read_to_string(path).map_err(|e| format!("{path:?}: {e}"))?;
    ron::from_str(&contents).map_err(|e| format!("{path:?}: {e}"))
}

/// Resolve a `Loadout` into a `UnitConfig` by loading hero and ability files from `data_dir`.
pub fn resolve_loadout(loadout: &Loadout, data_dir: &std::path::Path) -> Result<UnitConfig, String> {
    let hero_path = data_dir.join("heroes").join(format!("{}.ron", loadout.hero));
    let hero = load_hero_def(&hero_path)?;
    let mut config = UnitConfig::new(hero);
    for (ability_name, level) in &loadout.abilities {
        let ability_path = data_dir.join("abilities").join(format!("{}.ron", ability_name));
        let ability = load_ability_def(&ability_path)?;
        config.abilities.push((ability, *level));
    }
    Ok(config)
}

impl Effect {
    /// Whether this effect works on illusions.
    pub fn illusion_interaction(&self) -> IllusionInteraction {
        match self {
            // Crits work on illusions
            Effect::ChaosStrike { .. } => IllusionInteraction::Full,
            // Attack modifiers that do NOT work on illusions
            Effect::FurySwipes { .. } => IllusionInteraction::Disabled,
            Effect::EssenceShift { .. } => IllusionInteraction::Disabled,
            Effect::GlaivesOfWisdom { .. } => IllusionInteraction::Disabled,
            // All other effects: disabled by default for illusions
            _ => IllusionInteraction::Disabled,
        }
    }
}
