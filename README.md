# GladeDesk
Custom Console Idea

## Description
This is a more subtle plugin aimed at letting the user toy around with different values that create console emulations using math in the style of some of the older Airwindows plugins' code.
As someone fascinated by consoles and old hardware this is something I really wanted to make to play around with, but am now sharing with you!
![image](https://github.com/ardura/GladeDesk/assets/31751444/1cf154af-91d3-48a2-bdea-ebb963bebd05)


### Glade Desk consists of a few parts
- Input Gain
- Push amount (This is a sin distortion based on feeding more signal into the sin - it's subtle)
- Multiplier - This scales the coefficients and skews to really strain the sound
- Output Gain
- Wet/Dry Sum - This is actually Dry + (Processed*Wet) due to how the summation works in the console stuff
- Coefficient and Skew Sliders - I'm not too sure how to describe these, but this is meant as a plugin to be played with and heard to find the sound you like.

---
This plugin uses Rust with the Nih-plug crate!
