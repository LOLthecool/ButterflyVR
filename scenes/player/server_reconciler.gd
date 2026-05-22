extends Node

const MAX_SERVER_DISAGREE:float = 0.2 # todo: tune this

@export var player_access:PlayerAccess

var player:Player
var history:Array[Array]

func _ready() -> void:
	player = player_access.player
