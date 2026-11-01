[CmdletBinding()]
param([ValidateRange(2,600)][int]$Seconds=6,[ValidateRange(0,32)][int]$Save=2,[ValidateRange(0,60)][int]$DelaySeconds=0,
    [switch]$Mono,[ValidatePattern('^[a-z_0-9]+=[\w.,:\-]+$')][string[]]$Experiment=@())
# A PC check of the Wolfenstein producer (The New Order or The Old Blood, whichever runs: one producer, and monaka_play
# takes the one that runs), no headset: monaka_play capture attaches monaka_wolf_tno.dll to the running game for a few
# seconds and reads its channel with the channel probe. Prints the pair rate and the producer log; -Save keeps that
# many pairs as PNG in the run folder. -Mono leaves the engine's single view (both eyes get the whole frame).
# The game keeps rendering without focus.
# Build first: cargo build --release -p monaka -p monaka_wolf_tno; cargo build --release -p monaka_channel --example probe
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Check.ps1')
$arguments=@('capture','wolf_tno','--seconds',$Seconds,'--delay',$DelaySeconds)
if ($Save) { $arguments+=@('--save',$Save) }
if ($Mono) { $arguments+='stereo=0' }
Invoke-MonakaPlay @arguments @Experiment
