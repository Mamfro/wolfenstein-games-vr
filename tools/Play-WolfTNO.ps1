[CmdletBinding()]
param([ValidateRange(0.03,0.09)][double]$EyeSeparation=0.064,
    [ValidateRange(0.2,1.0)][double]$HudScale=0.35,[ValidateRange(0.5,20)][double]$HudDistance=2.0,
    [ValidateSet('Left','Right','None')][string]$DominantEye='Left',[ValidateSet('Lean','Full','Off')][string]$HeadPosition='Lean',[switch]$NoSightLock,
    [ValidateSet('Guns','Head','Off')][string]$Aim='Guns',
    [ValidatePattern('^[a-z_0-9]+=[\w.,:\-]+$')][string[]]$Experiment=@())
# Play Wolfenstein: The New Order or The Old Blood (whichever runs) in head-tracked stereo on the headset until the game exits: this runs monaka_play,
# which attaches the producer (monaka_wolf_tno.dll) and shows it with monaka_viewer. The game must be running (start it from Steam); runs restart if
# they end. Run folders go to monaka\runs.
# Uses the engine's own side-by-side stereo (both eyes every frame); you look and move where the head points
# (mouse/stick still turn). -Aim Guns (default): each gun follows its controller and fires where it points, one in each
# hand when dual wielding, the arms follow the guns, and the controllers are the game's pad (with two guns the triggers
# fire left and right). -Aim Head aims where you look; -Aim Off leaves aiming to the
# mouse/stick. God rays and lens flares are off (the engine draws them in the right eye only). The game
# keeps running without window focus while attached.
# The HUD and menus float in front of you: -HudScale is their height as a share of the view's
# height, -HudDistance how far away they float (metres). -DominantEye is the eye that looks straight down the
# weapon's sights (default Left). -HeadPosition: Lean (default) follows leaning but not the eye movement of
# turning the head; Off is rotation only; Full is everything the headset reports. While aiming down sights the eyes slide onto the
# game camera whatever -HeadPosition says, so the sights line up (-NoSightLock keeps head position there too). -Experiment passes extra producer options (key=value; see idt\wolf_tno\src\config.rs).
# The session itself is monaka_launch (crates\launch): this script only picks the running game and turns its parameters
# into monaka_play options, passing every one so its defaults stay these (monaka_play list shows them).
# Build first: cargo build --release -p monaka -p monaka_viewer -p monaka_wolf_tno
$ErrorActionPreference='Stop'
. (Join-Path $PSScriptRoot 'Monaka.ps1')
$play=Join-Path $MonakaRoot 'target\release\monaka_play.exe'
if (-not (Test-Path -LiteralPath $play)) { throw "Build first: cargo build --release -p monaka -p monaka_viewer -p monaka_wolf_tno (missing $play)" }
$invariant=[Globalization.CultureInfo]::InvariantCulture
function OnOff([bool]$on) { if ($on) { 'on' } else { 'off' } }
# One producer for both games; monaka_play takes the one that runs by its id.
$tno=Get-Process WolfNewOrder_x64 -ErrorAction SilentlyContinue
$tob=Get-Process WolfOldBlood_x64 -ErrorAction SilentlyContinue
if (-not $tno -and -not $tob) { throw 'Start Wolfenstein: The New Order or The Old Blood first.' }
if ($tno -and $tob) { throw 'Both Wolfenstein games are running; quit one.' }
$game=if ($tob) { 'wolf_tob' } else { 'wolf_tno' }
$aimOption=@{ Guns='hand_rig'; Head='head'; Off='off' }[$Aim]
& $play $game "aim=$aimOption" "eye_separation=$($EyeSeparation.ToString($invariant))" `
    "hud_scale=$($HudScale.ToString($invariant))" "hud_distance=$($HudDistance.ToString($invariant))" "dominant_eye=$($DominantEye.ToLowerInvariant())" `
    "head_position=$($HeadPosition.ToLowerInvariant())" "sight_lock=$(OnOff (-not $NoSightLock))" "extra=$($Experiment -join ' ')"
exit $LASTEXITCODE
