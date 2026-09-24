extends Button
class_name LicenseViewButton

@export var panel:PanelContainer
@export var license_label:Label

var license:String

func _pressed() -> void:
	if panel.visible:
		panel.visible = false
		return
	
	license_label.text = license
	panel.visible = true

func hide_panel() -> void:
	panel.visible = false
