; Error test file - contains intentional issues
; Used to test error handling and validation
G21
G90
G0 Z5
G0 X0 Y0
G1 Z-1 F200
G1 X10 F500
INVALID_COMMAND
G1 Y10
G1 X999999 Y999999 (out of range values)
G1 X0 Y0
G0 Z5
M30
