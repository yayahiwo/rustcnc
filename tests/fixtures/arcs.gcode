; Arc test program - circles and arcs
; Tests G2/G3 arc handling
G21
G90
G0 Z5
G0 X10 Y0

; Full clockwise circle (radius 10, center at origin)
G1 Z-1 F200
G2 X10 Y0 I-10 J0 F300

; Half counter-clockwise arc
G0 Z5
G0 X20 Y0
G1 Z-1 F200
G3 X20 Y20 I0 J10 F300

; Quarter arc
G0 Z5
G0 X0 Y30
G1 Z-0.5 F200
G2 X10 Y40 I10 J0 F400

G0 Z5
G0 X0 Y0
M30
