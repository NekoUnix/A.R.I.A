# VMC / OSC facial tracking (experimental development feature)

Choose **Tracking → VMC / OSC · face & head**. The default receive port is 39539.
Set the allowed sender IPv4 address, configure the sending app to send VMC to
this PC and port, then connect. Localhost stays loopback-only; an explicitly
configured LAN sender permits a LAN listener with sender-address filtering.

ARIA currently receives `/VMC/Ext/OK`, Head `/VMC/Ext/Bone/Pos`,
`/VMC/Ext/Blend/Val` and `/VMC/Ext/Blend/Apply` according to the
[published VMC protocol](https://protocol.vmc.info/english.html). ARKit-style
blendshape names feed the existing facial mappings. Head quaternion coordinates
are converted from Unity's handedness to ARIA's convention. Calibration and
custom mappings remain available; sender rest-pose conventions can differ.

Blend values commit on Apply. OSC bundles are bounded to four nesting levels,
256 messages and 16 KiB per UDP packet. A malformed bundle cannot partially
replace the pose. Non-finite values and invalid quaternions are rejected. State
resets after a packet gap; tracking freshness uses local receipt time. Stopping
the receiver releases its UDP port.

This adapter does not yet retarget body/finger bones, consume camera or light
controls, load remote files or execute keyboard messages. Hardware/sender-app
acceptance is separate from synthetic protocol and local UDP tests. It is not
advertised as full VMC compatibility.
