struct CameraUniforms {
    origin: vec3<f32>,
    u: vec3<f32>,
    v: vec3<f32>,
    w: vec3<f32>,
}

struct Uniforms {
    width: u32,
    height: u32,
    frame_count: u32,
    camera: CameraUniforms, 
}

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var radiance_samples: texture_storage_2d<rgba32float, read_write>;

struct VertexInput {
    @location(0) index: u32,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    var pos = vec2<f32>(0.0, 0.0);
    if (input.index == 0u || input.index == 3u) { pos = vec2<f32>(-1.0, -1.0); }
    else if (input.index == 1u) { pos = vec2<f32>(1.0, -1.0); }
    else if (input.index == 2u || input.index == 4u) { pos = vec2<f32>(1.0, 1.0); }
    else if (input.index == 5u) { pos = vec2<f32>(-1.0, 1.0); }
    
    out.position = vec4<f32>(pos, 0.0, 1.0);
    out.uv = pos * 0.5 + 0.5;
    return out;
}

var<private> rng_state: u32;

fn init_rng(pixel: vec2<u32>, frame: u32) {
    rng_state = (pixel.x + pixel.y * uniforms.width) ^ (frame * 719393u);
}

fn rand() -> f32 {
    let state = rng_state * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    rng_state = (word >> 22u) ^ word;
    return f32(rng_state) / 4294967295.0;
}

fn random_in_unit_sphere() -> vec3<f32> {
    for (var i = 0; i < 10; i++) {
        let p = 2.0 * vec3<f32>(rand(), rand(), rand()) - vec3<f32>(1.0);
        if (dot(p, p) < 1.0) {
            return p;
        }
    }
    return normalize(vec3<f32>(rand(), rand(), rand()));
}

fn aces_tone_map(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}

struct Ray {
    origin: vec3<f32>,
    direction: vec3<f32>,
}

struct HitRecord {
    t: f32,
    p: vec3<f32>,
    normal: vec3<f32>,
    mat_type: u32,
    hit: bool,
}

fn hit_sphere(center: vec3<f32>, radius: f32, r: Ray, t_min: f32, t_max: f32, mat_type: u32) -> HitRecord {
    var rec: HitRecord;
    rec.hit = false;
    
    let oc = r.origin - center;
    let a = dot(r.direction, r.direction);
    let b = 2.0 * dot(oc, r.direction);
    let c = dot(oc, oc) - radius * radius;
    let discriminant = b*b - 4.0*a*c;
    
    if (discriminant > 0.0) {
        let root = sqrt(discriminant);
        var temp = (-b - root) / (2.0 * a);
        if (temp < t_max && temp > t_min) {
            rec.t = temp;
            rec.p = r.origin + rec.t * r.direction;
            rec.normal = (rec.p - center) / radius;
            rec.hit = true;
            rec.mat_type = mat_type;
            return rec;
        }
        temp = (-b + root) / (2.0 * a);
        if (temp < t_max && temp > t_min) {
            rec.t = temp;
            rec.p = r.origin + rec.t * r.direction;
            rec.normal = (rec.p - center) / radius;
            rec.hit = true;
            rec.mat_type = mat_type;
            return rec;
        }
    }
    return rec;
}

fn world_hit(r: Ray) -> HitRecord {
    var closest: HitRecord;
    closest.hit = false;
    closest.t = 1e30;

    let rec1 = hit_sphere(vec3<f32>(0.0, 0.0, -1.0), 0.5, r, 0.001, closest.t, 3u);
    if (rec1.hit) { closest = rec1; }

    let rec2 = hit_sphere(vec3<f32>(0.0, 0.0, -1.0), -0.45, r, 0.001, closest.t, 3u);
    if (rec2.hit) { closest = rec2; }

    let rec3 = hit_sphere(vec3<f32>(-1.1, 0.0, -1.0), 0.5, r, 0.001, closest.t, 2u);
    if (rec3.hit) { closest = rec3; }

    let rec4 = hit_sphere(vec3<f32>(1.1, 0.0, -1.0), 0.5, r, 0.001, closest.t, 1u);
    if (rec4.hit) { closest = rec4; }

    let rec_g = hit_sphere(vec3<f32>(0.0, -100.5, -1.0), 100.0, r, 0.001, closest.t, 0u);
    if (rec_g.hit) { closest = rec_g; }

    return closest;
}

fn ray_color(r_in: Ray) -> vec3<f32> {
    var cur_ray = r_in;
    var cur_attenuation = vec3<f32>(1.0, 1.0, 1.0);
    
    for (var depth = 0; depth < 50; depth++) {
        let rec = world_hit(cur_ray);
        
        if (rec.hit) {
            var scattered_origin = rec.p;
            var scattered_direction = vec3<f32>(0.0);
            var attenuation = vec3<f32>(0.0);
            
            if (rec.mat_type == 3u) {
                let ir = 1.5;
                var refraction_ratio = ir;
                var normal_vec = -rec.normal;
                
                if (dot(cur_ray.direction, rec.normal) < 0.0) {
                    refraction_ratio = 1.0 / ir;
                    normal_vec = rec.normal;
                }
                
                let unit_dir = normalize(cur_ray.direction);
                let cos_theta = min(dot(-unit_dir, normal_vec), 1.0);
                let sin_theta = sqrt(1.0 - cos_theta * cos_theta);
                
                let cannot_refract = refraction_ratio * sin_theta > 1.0;
                let r0 = (1.0 - ir) / (1.0 + ir);
                let r0_sq = r0 * r0;
                let reflectance = r0_sq + (1.0 - r0_sq) * pow(1.0 - cos_theta, 5.0);
                
                if (cannot_refract || reflectance > rand()) {
                    scattered_direction = reflect(unit_dir, normal_vec);
                } else {
                    let r_out_perp = refraction_ratio * (unit_dir + cos_theta * normal_vec);
                    let r_out_parallel = -sqrt(abs(1.0 - dot(r_out_perp, r_out_perp))) * normal_vec;
                    scattered_direction = r_out_perp + r_out_parallel;
                }
                attenuation = vec3<f32>(1.0, 1.0, 1.0);
            } 
            else if (rec.mat_type == 1u) {
                let fuzz = 0.0; 
                let reflected = reflect(normalize(cur_ray.direction), rec.normal);
                scattered_direction = reflected + fuzz * random_in_unit_sphere();
                attenuation = vec3<f32>(0.7, 0.6, 0.5); 
                if (dot(scattered_direction, rec.normal) <= 0.0) { return vec3<f32>(0.0); }
            } 
            else if (rec.mat_type == 2u) {
                let scatter_target = rec.p + rec.normal + random_in_unit_sphere();
                scattered_direction = scatter_target - rec.p;
                attenuation = vec3<f32>(0.7, 0.3, 0.3); 
            }
            else {
                let scatter_target = rec.p + rec.normal + random_in_unit_sphere();
                scattered_direction = scatter_target - rec.p;
                let sines = sin(3.0 * rec.p.x) * sin(3.0 * rec.p.z);
                if (sines < 0.0) { attenuation = vec3<f32>(0.2, 0.2, 0.2); } 
                else { attenuation = vec3<f32>(0.9, 0.9, 0.9); }
            }

            cur_ray = Ray(scattered_origin, normalize(scattered_direction));
            cur_attenuation = cur_attenuation * attenuation;
        } else {
            let unit_dir = normalize(cur_ray.direction);
            let t = 0.5 * (unit_dir.y + 1.0);
            let sky = (1.0 - t) * vec3<f32>(1.0, 1.0, 1.0) + t * vec3<f32>(0.5, 0.7, 1.0);
            return cur_attenuation * sky;
        }
    }
    return vec3<f32>(0.0, 0.0, 0.0);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coord = vec2<u32>(vec2<i32>(in.position.xy));
    init_rng(coord, uniforms.frame_count);

    let resolution = vec2<f32>(f32(uniforms.width), f32(uniforms.height));
    let aspect_ratio = resolution.x / resolution.y;
    
    let jitter = vec2<f32>(rand() - 0.5, rand() - 0.5);
    let uv = (in.position.xy + jitter) / resolution;
    
    let p = (uv * 2.0 - 1.0);
    let screen_p = vec2<f32>(p.x * aspect_ratio, -p.y); 
    
    let cam = uniforms.camera;
    let ray_dir = normalize(cam.w + cam.u * screen_p.x + cam.v * screen_p.y);
    let r = Ray(cam.origin, ray_dir);

    let color = ray_color(r);
    
    var acc_color = vec4<f32>(0.0);
    if (uniforms.frame_count > 1u) {
        acc_color = textureLoad(radiance_samples, vec2<i32>(coord));
    }
    
    var safe_color = color;
    if (any(color != color)) { safe_color = vec3<f32>(0.0); }

    let new_acc = acc_color + vec4<f32>(safe_color, 1.0);
    textureStore(radiance_samples, vec2<i32>(coord), new_acc);
    
    let accumulated_linear = new_acc.rgb / f32(uniforms.frame_count);
    
    let tone_mapped = aces_tone_map(accumulated_linear);
    let gamma_corrected = pow(tone_mapped, vec3<f32>(1.0/2.2));
    
    return vec4<f32>(gamma_corrected, 1.0);
}