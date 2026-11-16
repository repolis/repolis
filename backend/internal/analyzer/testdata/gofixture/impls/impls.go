package impls

// Unnamed receivers on types sharing method names: the shape that used to make
// one type absorb every same-named method in the file.

type jsonCodec struct{}
type xmlCodec struct{}
type yamlCodec struct{}

func (jsonCodec) Name() string { return "json" }
func (jsonCodec) Ext() string  { return ".json" }
func (xmlCodec) Name() string  { return "xml" }
func (xmlCodec) Ext() string   { return ".xml" }
func (yamlCodec) Name() string { return "yaml" }
func (yamlCodec) Ext() string  { return ".yaml" }
