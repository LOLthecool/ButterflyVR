extends Node

const MAX_SERVER_DISAGREE:float = 0.2 # todo: tune this

@export var player_access:PlayerAccess

var player:Player
var history:Array[Array]

class TickInfo:
	var position:Vector3
	var rotation:Vector3
	var velocity:Vector3

func make_tick() -> TickInfo:
	var item:TickInfo = TickInfo.new()
	item.position = player.position
	item.rotation = player.rotation
	item.velocity = player.velocity
	return item

func _ready() -> void:
	player = player_access.player

func _physics_process(delta: float) -> void:
	if history.is_empty():
		history.push_back(make_tick())
		return
	
