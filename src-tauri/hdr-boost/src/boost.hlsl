Texture2D<float4> desktop : register(t0);
cbuffer Settings : register(b0) {
    float gain;
    float ceiling;
    float split;
    float width;
};

float4 vertexMain(uint id : SV_VertexID) : SV_Position {
    float2 corner = float2((id << 1) & 2, id & 2);
    return float4(corner * float2(2, -2) + float2(-1, 1), 0, 1);
}

float4 pixelMain(float4 position : SV_Position) : SV_Target {
    float3 original = max(desktop.Load(int3(int2(position.xy), 0)).rgb, 0);
    float appliedGain = split > 0.5 && position.x < width * 0.5 ? 1.0 : gain;
    float3 bright = original * appliedGain;
    // Preserve black and color ratios while limiting output to the tested HDR range.
    float maximum = max(bright.r, max(bright.g, bright.b));
    if (maximum > ceiling) bright *= ceiling / maximum;
    return float4(bright, 1);
}
