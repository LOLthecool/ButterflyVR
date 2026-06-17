extends Node

var effect:AudioEffectCapture
var is_recording:bool = false

func _ready() -> void:
	var idx:int = AudioServer.get_bus_index("Record")
	effect = AudioServer.get_bus_effect(idx, 0)

#func _physics_process(_delta: float) -> void:
	#while is_recording:
		#var tmp:PackedVector2Array = effect.get_buffer(960)
		#if tmp.size() == 0:
			#break
	#	NetworkManager.transmit_audio(tmp)
	#((get_child(1) as AudioStreamPlayer).stream as VoiceStream).get_current_playback().buffer_audio(NetworkManager.get_audio())
