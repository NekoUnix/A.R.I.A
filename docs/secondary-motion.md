# Hair, tails and clothing physics

VRM and VRC / GLB avatars now start with **Medium** secondary motion. Existing
custom settings are kept when you reopen an avatar. Tracking, idle movement and
clicked gestures move the body; flexible chains lag behind, swing and settle.
Each loaded avatar runs its own simulation, shared by its stage and OBS outputs.

## Get started

1. Import a VRM or skinned GLB using **Profiles → + Add avatar**.
2. Select that avatar's stage tab. Open **Avatar → Spring physics** (Springs in the Inspector).
3. Leave **Enable spring bones** checked and move your head gently. No phone-side
   physics setup is needed; your existing tracking source supplies the movement.
4. Choose **Soft**, **Medium** or **Firm**. These buttons replace overall and group
   tuning for this avatar, retaining bone detection and manual roots.
5. Open **Spring groups** to adjust or disable individual chains. Group names
   come from the export or the detected root bone, never a hard-coded test model.
6. Click **Save physics for this avatar** or **Save profile**. Edits also save
   automatically. Movement/pose presets include physics and lighting.

```text
Face tracking + body idle / gesture
                 ↓
        Rigid humanoid skeleton
                 ↓
   Flexible hair / tail / clothing chains
        lag → swing → return to rest
                 ↓
     One pose shared by all OBS outputs
```

## Choose the feel

| Control | What it does |
| --- | --- |
| Motion strength | Blends the simulated bend into the body pose. Zero removes its visible motion; values above one currently cap at the full simulated bend. |
| Inertia | Carries momentum. Raise gently for more follow-through. A stability cap prevents unlimited velocity. |
| Stiffness / response | Pulls the chain back toward its resting direction. Higher is firmer; lower is softer. |
| Gravity multiplier | Scales exported gravity. A VRM joint with zero authored gravity remains unaffected by this control. Generated chains have a small gravity force. |
| Side wind | Adds a steady sideways force; zero means no wind. |
| Extra damping | Removes additional momentum. Higher settles sooner and bounces less. |
| Maximum bend | Limits each joint's bend relative to its current posed parent. Medium is 45 degrees. Zero holds the joint rigid. |
| Use collision shapes | Enables collision response for this group: exported shapes for authored VRM groups, generated body and flexible-part envelopes for generated groups. |
| Settle motion | Clears momentum and starts from the current body pose. Settings are retained. |

Global strength/inertia/response/gravity multiply the matching group values.
Damping, bend limits and collision enable inherit the overall values unless
**Custom damping, swing and collisions** is checked for that group. Medium adds
0.15 damping, allows 45 degrees and enables collisions. Resetting one
group restores inheritance; clicking an overall preset resets all group tuning.

## If hair stays rigid or the wrong thing moves

ARIA retains exported VRM spring groups. **Find secondary bones → Guess flexible
bones automatically** recognizes breasts, belly, buttocks, hair, tails, animal ears,
earrings, straps and other clothing/accessories on the active skinned skeleton.
Flexible body groups start with a firmer response and less gravity than hair.
Small rotations are preserved, so gentle idle/tracking movement can drive both
sides of a paired rig without needing higher wind or motion strength. Each side
still responds independently to its own rig, weights, contacts and saved tuning;
ARIA does not copy one side's pose onto the other.
Head, spine, limbs, eyes, fingers, facial controls, twist helpers and their ancestors
keep their tracking/rig roles. Existing authored chains are not duplicated.

Open **Bone inspector · all imported bones** and search a name such as `Breast`.
It lists every skin bone, skeleton parent and exported endpoint, including bones
that do not need their own simulation. Hover the status for its parent, mesh
influence and endpoint source. **Auto** uses the guessed role. **Simulate** adds an
eligible branch with unfamiliar names; **Keep rigid** excludes that branch and its
children from generated physics. Auto clears that bone's override, while an
ancestor's Keep rigid still applies. Use **Spring groups** to tune strength,
inertia, response, damping and bend, or disable an exported VRM group. Choices save
per avatar with profiles and presets. Up to 128 manual roots may be selected.

If only one side moves, search for both left and right bone names in the inspector.
Confirm that neither branch is **Keep rigid**, both spring groups are enabled,
and neither has zero motion strength or maximum bend. Moving just the head mainly
drives chains attached to the head; use **Life & idle movement** or torso tracking
to exercise body chains. **Medium** restores overall and group tuning, but keeps
bone overrides: use **Auto** on an accidentally excluded bone separately. Save
the profile after any intentional changes. Updating ARIA preserves your settings.

Unweighted end bones now supply the correct chain length. A single weighted bone
with no child can use a bounded virtual endpoint estimated from its weighted mesh;
the inspector identifies these estimates. Unknown bone names are left following
their parents until reviewed. Branching containers anchor separate strands.
All imported skin bones already participate in mesh deformation; making every
bone spring independently would interfere with tracking and the skeleton's rig.

A flat mesh without skin weights or flexible bones cannot bend automatically.
Ask the model author for a rigged export in that case. ARIA's generated chains
are **experimental approximations**, not imported Unity/VRChat PhysBone components.
Plain GLB does not supply those component settings. The collision envelopes below
are ARIA estimates, not imported Unity settings or exact mesh surfaces.

## Body and self collision

Open **Body & self collision** inside Overall secondary motion. New and older
profiles default to generated body and flexible-group collision enabled; your
existing **Use collision shapes** setting and per-group overrides still apply.

- **Generate body collision shapes** fits capsules to weighted head, torso and
  limb regions using the humanoid assignments. Shapes move with those bones.
  The panel reports how many were fitted; missing body assignments can be fixed
  in GLB rig & tracking.
- **Collide with other flexible groups** adds moving envelopes for nearby breast,
  hair and accessory regions. A branch never collides with itself or its ancestors.
  Up to 256 flexible envelopes are kept, prioritizing larger regions on huge rigs.
- **Body collision size** enlarges or shrinks fitted body shapes (0.5–1.5).
  **Spring thickness** adds a collision radius as a fraction of bone length.
  Increase gently if artwork still clips; decrease if motion feels crowded.
- The contact counter reports corrections from the latest physics update. It is
  not a count of unique mesh intersections. Turning off the group collision toggle
  removes its response. All settings save per avatar and in presets.

The solver checks generated bone segments as well as tips, catches fast tip
crossings and reapplies length/bend constraints during contact correction. It
preserves overlaps already present in the current rigged pose, including relaxed
arms and gestures, instead of forcing clothing away from the body to recover the
export's T-pose. All groups sample a consistent collider pose for each physics
step. Strength blends the resulting pose toward rest; values below one can reduce
visible contact accuracy.
Authored VRM groups retain their supplied shapes and gain the corrected solver.

**Life & idle movement** strengths, speed and on/off changes blend smoothly. You
can keep **Arms & elbows** at 2 without the setting itself instantly kicking the
clothing. Setting a strength to zero gently settles that component; it does not
disable collision or other secondary movement. About 95% of a setting change is
applied within half a second, independently of display frame rate.

These are approximate **bone collision envelopes**, not triangle-mesh cloth
simulation. Hair strands within the same branch, thin garments and artwork outside
the fitted shapes can still intersect. Rigid tracking/gesture poses are not moved
by spring collision: use Relax arms/gesture strength if the hands or arms cross the
body. No setting can repair missing skin weights or overlapping source geometry.

**Freeze pose** stops the solver and saves the final spring rotations for screenshots.
Resuming starts fresh momentum. A long pause or large jump resets unstable motion.
The solver advances on every rendered frame using substeps no longer than 8.3 ms.
Tracking poses are interpolated across these steps; drag uses elapsed time and
strength blends the displayed pose once. High-refresh displays do not hold an old
60 Hz pose or repeatedly suppress weak motion. Tiny
links retain bounded lag behind their parents. Generated collision recovery limits
angular speed and preserves sliding momentum instead of kicking a joint or stopping
all its motion on contact. Small direction changes survive contact recovery too.
Generated shapes use precise contact boundaries without repeated outward nudges,
so close-fitting accessories can settle without a motion dead zone.
Conflicting approximate shapes recover gradually, so
brief intersections are still possible; they do not promise mesh-level cloth contact.
Pose/collider scratch buffers are reused. Turning physics off removes simulation
work, and all outputs share the same computed pose.
