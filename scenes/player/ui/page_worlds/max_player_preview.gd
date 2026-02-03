extends Label

@export var max_player_slider:HSlider

func _process(_delta: float) -> void:
	text = str(int(max_player_slider.value))
