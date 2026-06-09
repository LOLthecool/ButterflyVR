extends Node

const MAX_SERVER_DISAGREE:float = 0.02 # todo: tune this
const MAX_SERVER_DISAGREE_SQUARED:float = MAX_SERVER_DISAGREE * MAX_SERVER_DISAGREE # compare squared lengths
const MAX_RECOVERY_SPEED:float = 50
const max_VERTICAL_RECOVERY_SPEED:float = 25
const BASE_LATENCY:int = 8

@export var player_access:PlayerAccess

var player:Player

var position_history:Queue = Queue.new()
var velocity_history:Queue = Queue.new()

func _ready() -> void:
	player = player_access.player

func _physics_process(delta: float) -> void:
	if player.server_position == Vector3.ZERO:
		return
	
	if !NetworkManager.is_running():
		return
	
	var rtt_ticks:int = ceilf(NetworkManager.get_highest_rtt_millis() / (delta * 1000)) as int
	while position_history.size() > (rtt_ticks * 2) + BASE_LATENCY:
		position_history.pop_front()
		velocity_history.pop_front()
	
	position_history.push_back(player.position)
	velocity_history.push_back(player.velocity) 
	
	var server_position:Vector3 = player.server_position
	var closest_match_diff_length_squared:float = INF
	var closest_match_index:int = -1
	
	for idx:int in position_history.size():
		var current_tick_diff:Vector3 = abs(position_history.get(idx) - server_position)
		var current_tick_length_diff_squared:float = current_tick_diff.length_squared()
		
		if current_tick_length_diff_squared > MAX_SERVER_DISAGREE_SQUARED:
			if current_tick_length_diff_squared < closest_match_diff_length_squared:
				closest_match_diff_length_squared = current_tick_length_diff_squared
				closest_match_index = idx
		else:
			return # close enough to server position
	
	if closest_match_index == -1:
		return
	
	# no reconcil behavior yet so we just log
	push_warning(
			"player position disagree. 
			server pos: %s, 
			closest client pos: %s 
			occured %s ticks ago. 
			current client pos %s" % [
				server_position, 
				position_history.get(closest_match_index), 
				position_history.size() - closest_match_index, 
				player.position])
