; Simple test program - 10mm square at Z-1
; Expected: moves to (10,10,-1) and back
G21 (metric)
G90 (absolute)
G0 Z5 (safe height)
G0 X0 Y0
G1 Z-1 F200
G1 X10 F500
G1 Y10
G1 X0
G1 Y0
G0 Z5
G0 X0 Y0
M30
