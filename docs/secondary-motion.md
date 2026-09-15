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
| Use exported body colliders | Enables the file's sphere/capsule collision shapes when supplied. It does not create body colliders for plain GLB. |
| Settle motion | Clears momentum and starts from the current body pose. Settings are retained. |

Global strength/inertia/response/gravity multiply the matching group values.
Damping, bend limits and collision enable inherit the overall values unless
**Custom damping, swing and collisions** is checked for that group. Medium adds
0.15 damping, allows 45 degrees and enables exported collisions. Resetting one
group restores inheritance; clicking an overall preset resets all group tuning.

## If hair stays rigid or the wrong thing moves

ARIA retains exported VRM spring groups. **Find secondary bones → Automatically
detect hair, tails, ears and clothing** adds missing named chains on the active
skinned skeleton. Head, spine, limbs, eyes and their ancestors are protected from
automatic simulation. Existing spring chains are not added a second time.
Branching roots anchor multiple strands. End bones move with their parent.

For unusual names, choose **Add a bone chain…** and select the uppermost flexible
bone. The chain needs at least two bones. A rigid humanoid bone cannot be used as
a manual spring root. Remove a manual chain or disable an unwanted group to undo
it. A chain already covered by exported physics is not duplicated.

A flat mesh without skin weights or flexible bones cannot bend automatically.
Ask the model author for a rigged export in that case. ARIA's generated chains
are **experimental approximations**, not imported Unity/VRChat PhysBone components.
Plain GLB does not supply those component settings. Collision-free GLB chains can
intersect the body; reduce bend/strength or use a VRM export with authored colliders.
There is no cloth mesh, self-collision or rigid-body simulation in this feature.

**Freeze pose** stops the solver and saves the final spring rotations for screenshots.
Resuming starts fresh momentum. A long pause or large jump resets unstable motion.
The solver uses bounded 60 Hz steps and reuses its rotation scratch buffer. Turning
physics off removes simulation work, and all outputs share the same computed pose.
