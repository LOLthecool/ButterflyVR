extends Node
class_name Highlighter

var mat:StandardMaterial3D = StandardMaterial3D.new()

var geometry:MeshInstance3D
var highlight_geometry:MeshInstance3D

func setup() -> void:
	if geometry == null:
		queue_free()
		return
	highlight_geometry = MeshInstance3D.new()
	geometry.add_child(highlight_geometry)
	
	highlight_geometry.visible = false
	
	highlight_geometry.mesh = geometry.mesh
	
	mat.albedo_color = Color(0.0, 0.0, 1.0)
	mat.cull_mode = BaseMaterial3D.CULL_FRONT
	mat.shading_mode = BaseMaterial3D.SHADING_MODE_UNSHADED
	
	highlight_geometry.set_surface_override_material(0, mat)
	
	highlight_geometry.scale = Vector3(1.05, 1.05, 1.05)
	highlight_geometry.position = Vector3.ZERO
	highlight_geometry.quaternion = Quaternion.IDENTITY

func start_highlight() -> void:
	highlight_geometry.visible = true

func end_highlight() -> void:
	highlight_geometry.visible = false
