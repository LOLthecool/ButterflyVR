extends Node

const MAX_SERVER_DISAGREE:float = 0.05 # todo: tune this
const MAX_SERVER_DISAGREE_SQUARED:float = MAX_SERVER_DISAGREE * MAX_SERVER_DISAGREE # compare squared lengths
const MAX_RECOVERY_SPEED:float = 50
const max_VERTICAL_RECOVERY_SPEED:float = 25

@export var player_access:PlayerAccess

var player:Player

# using Basis to store ticks is not meaningful it is just used as an optimisation
# otherwise we would need to instance a class every time
var history:Queue = Queue.new()

func make_tick() -> Basis:
	return Basis(player.position, player.rotation, player.velocity)

func make_server_tick() -> Basis:
	return Basis(player.server_position, player.server_rotation, player.server_velocity)

func _ready() -> void:
	player = player_access.player

func _physics_process(delta: float) -> void:
	var rtt_ticks:int = ceilf(NetworkManager.get_highest_rtt_millis() / (delta * 1000)) as int
	while history.size() > rtt_ticks * 2:
		history.pop_back()
	
	history.push_front(make_tick())
	
	var server_tick:Basis = make_server_tick()
	var closest_match_diff_length_squared:float = INF
	var closest_match_index:int = -1
	
	for idx:int in history.size():
		var current_tick:Basis = history.get(idx)
		var diff:Basis = Basis(
				current_tick.x - server_tick.x, 
				current_tick.y - server_tick.y, 
				current_tick.z - server_tick.z)
		
		@warning_ignore("unsafe_cast")
		var diff_length:float = ((abs(diff.x) + abs(diff.y) + abs(diff.z)
				) as Vector3).length_squared()
		
		if diff_length < closest_match_diff_length_squared:
			if diff_length > MAX_SERVER_DISAGREE_SQUARED:
				closest_match_diff_length_squared = diff_length
				closest_match_index = idx
			else:
				return
	
	# couldnt find a close enough match, must reconcil
	push_warning(
			"player position disagree. 
			server pos: %s, 
			closest client pos: %s 
			occured %s ticks ago. 
			current client pos %s" % [
				server_tick.x, 
				history.get(closest_match_index).x, 
				history.size() - closest_match_index, 
				player.position])
