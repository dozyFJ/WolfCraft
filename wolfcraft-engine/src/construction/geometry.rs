//! Shared render geometry and conservative, segmented collision for building pieces.
//! Coordinates are local (i,j,k), with k up on every cube-sphere face.
use super::{Form, FORM_COUNT};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface { Main, Fabric, Glass, Metal, Light, Mirror, Ceramic, Plant, Front, Top, Picture }
#[derive(Clone)]
pub struct Quad { pub points: [[f32; 3]; 4], pub surface: Surface }
#[derive(Default)]
pub struct Geometry { pub boxes: Vec<[f32; 6]>, pub faces: Vec<Quad> }

pub fn model(form: Form, rotation: u8, joined: u8) -> &'static Geometry {
    static CACHE: OnceLock<Vec<OnceLock<Geometry>>> = OnceLock::new();
    let slots = CACHE.get_or_init(|| (0..FORM_COUNT * 16).map(|_| OnceLock::new()).collect());
    slots[form as usize * 16 + (rotation % 4) as usize * 4 + (joined & 3) as usize].get_or_init(|| {
        let mut g = build(form, joined);
        for face in &mut g.faces { for p in &mut face.points { *p = rotate(*p, rotation); } }
        for b in &mut g.boxes {
            let points = [rotate([b[0], b[1], b[2]], rotation), rotate([b[3], b[1], b[2]], rotation), rotate([b[0], b[4], b[5]], rotation), rotate([b[3], b[4], b[5]], rotation)];
            *b = bounds(&points);
        }
        g
    })
}
fn rotate(mut p: [f32;3], r: u8) -> [f32;3] { for _ in 0..r%4 { p = [1.0-p[1], p[0], p[2]]; } p }
fn bounds(points: &[[f32;3]]) -> [f32;6] {
    let mut b = [1.0f32,1.0,1.0,0.0,0.0,0.0];
    for p in points { for a in 0..3 { b[a]=b[a].min(p[a]).clamp(0.0,1.0); b[a+3]=b[a+3].max(p[a]).clamp(0.0,1.0); }}
    b
}
impl Geometry {
    fn face(&mut self, points: [[f32;3];4], surface: Surface) { self.faces.push(Quad { points, surface }); }
    fn cuboid(&mut self, b: [f32;6], surface: Surface) {
        self.boxes.push(b);
        let [x,y,z,X,Y,Z]=b;
        let p=[[x,y,z],[X,y,z],[X,Y,z],[x,Y,z],[x,y,Z],[X,y,Z],[X,Y,Z],[x,Y,Z]];
        for f in [[0,3,2,1],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]] { self.face(f.map(|i|p[i]),surface); }
    }
    /// A box that is only drawn (no collision box): the part of big furniture that reaches
    /// out of its own cell (the far half of a bed, the top of a wardrobe).
    fn visual(&mut self, b: [f32;6], surface: Surface) {
        self.cuboid(b, surface);
        self.boxes.pop();
    }
    fn wedge(&mut self) {
        use Surface::Main;
        self.face([[0.,0.,0.],[1.,0.,0.],[1.,1.,0.],[0.,1.,0.]], Main);
        self.face([[0.,0.,0.],[1.,0.,1.],[1.,1.,1.],[0.,1.,0.]], Main);
        self.face([[1.,0.,0.],[1.,0.,1.],[1.,1.,1.],[1.,1.,0.]], Main);
        self.face([[0.,0.,0.],[1.,0.,0.],[1.,0.,1.],[1.,0.,1.]], Main);
        self.face([[0.,1.,0.],[1.,1.,1.],[1.,1.,0.],[1.,1.,0.]], Main);
        for n in 0..16 { let x=n as f32/16.; self.boxes.push([x,0.,0.,x+1./16.,1.,x+1./16.]); }
    }
    fn arc(&mut self, radius: f32, angle: f32) {
        let steps=(angle/7.5).ceil() as usize;
        let inner=radius-0.14;
        let p=|r:f32,a:f32,z:f32|[r*a.cos(),r*a.sin(),z];
        for n in 0..steps {
            let a=(angle*n as f32/steps as f32).to_radians(); let b=(angle*(n+1) as f32/steps as f32).to_radians();
            let points=[p(radius,a,0.),p(radius,b,0.),p(radius,b,1.),p(radius,a,1.),p(inner,a,0.),p(inner,b,0.),p(inner,b,1.),p(inner,a,1.)];
            for f in [[0,1,2,3],[4,7,6,5],[0,4,5,1],[3,2,6,7]] {self.face(f.map(|i|points[i]),Surface::Main);}
            if n==0 { self.face([points[0],points[3],points[7],points[4]],Surface::Main); }
            if n+1==steps { self.face([points[1],points[5],points[6],points[2]],Surface::Main); }
            self.boxes.push(bounds(&points));
        }
    }
    fn dome(&mut self, span: f32, radius:f32, center:f32) {
        let p=|r:f32,a:f32,b:f32|[center+r*a.cos()*b.cos(),center+r*a.sin()*b.cos(),r*b.sin()];
        let nx=(span/15.).ceil() as usize;
        for y in 0..6 {for x in 0..nx {
            let a=(span*x as f32/nx as f32).to_radians();let A=(span*(x+1) as f32/nx as f32).to_radians();
            let b=(y as f32*15.).to_radians();let B=((y+1) as f32*15.).to_radians();
            let outer=[p(radius,a,b),p(radius,A,b),p(radius,A,B),p(radius,a,B)];
            let inner=[p(radius-0.14,a,b),p(radius-0.14,A,b),p(radius-0.14,A,B),p(radius-0.14,a,B)];
            self.face(outer,Surface::Main);self.face([inner[3],inner[2],inner[1],inner[0]],Surface::Main);
            if y==0 { self.face([outer[0],inner[0],inner[1],outer[1]],Surface::Main); }
            if x==0 { self.face([outer[0],outer[3],inner[3],inner[0]],Surface::Main); }
            if x+1==nx { self.face([outer[1],inner[1],inner[2],outer[2]],Surface::Main); }
            let mut all=outer.to_vec();all.extend(inner);self.boxes.push(bounds(&all));
        }}
    }
}
fn build(form: Form, joined: u8) -> Geometry {
    use Form::*; use Surface::Main;
    let mut g=Geometry::default();
    match form {
        StorageChest|Barrel|PortableCrate|SharedChest => {
            g.cuboid([0.02,0.06,0.,0.98,0.96,0.75],Main);g.cuboid([0.01,0.05,0.75,0.99,0.97,0.93],Main);
            g.cuboid([0.43,0.01,0.58,0.57,0.06,0.83],if form==SharedChest {Surface::Light}else{Surface::Metal});
            if form==PortableCrate {for x in [0.16,0.76] {g.cuboid([x,0.03,0.,x+0.07,0.98,0.94],Surface::Metal);}}
        }
        ItemFrame|PaintingForest|PaintingSky|PaintingAbstract => {
            g.cuboid([0.03,0.05,0.03,0.97,0.12,0.97],Main);
            for b in [[0.,0.,0.,0.08,0.05,1.],[0.92,0.,0.,1.,0.05,1.],[0.08,0.,0.,0.92,0.05,0.08],[0.08,0.,0.92,0.92,0.05,1.]] {g.cuboid(b,Main);}
            if form!=ItemFrame {g.face([[0.08,0.035,0.08],[0.92,0.035,0.08],[0.92,0.035,0.92],[0.08,0.035,0.92]],Surface::Picture);}
        }
        Workbench|Furnace|Terminal|Command => {
            g.cuboid([0.,0.,0.,1.,1.,1.],Main);
            g.faces[1].surface=Surface::Top;g.faces[2].surface=Surface::Front;
        }
        StoneAltar|RootAltar|EmberAltar => {
            g.cuboid([0.,0.,0.,1.,1.,0.2],Main);g.cuboid([0.23,0.23,0.2,0.77,0.77,0.65],Main);
            g.cuboid([0.14,0.14,0.65,0.86,0.86,0.75],Main);g.cuboid([0.36,0.36,0.75,0.64,0.64,0.95],Surface::Light);
        }
        LowerSlab => g.cuboid([0.,0.,0.,1.,1.,0.5],Main),
        UpperSlab => g.cuboid([0.,0.,0.5,1.,1.,1.],Main),
        VerticalSlab => g.cuboid([0.,0.,0.,0.5,1.,1.],Main),
        Stairs|InnerStairs|OuterStairs => {
            g.cuboid([0.,0.,0.,1.,1.,0.5],Main);
            if form==OuterStairs {g.cuboid([0.5,0.5,0.5,1.,1.,1.],Main);}
            else {g.cuboid([0.,0.5,0.5,1.,1.,1.],Main);if form==InnerStairs {g.cuboid([0.5,0.,0.5,1.,0.5,1.],Main);}}
        }
        Wedge => {g.wedge();for face in &mut g.faces {for p in &mut face.points {p.swap(0,1);}face.points.reverse();}for b in &mut g.boxes {b.swap(0,1);b.swap(3,4);}},
        Beam => g.cuboid([0.35,0.,0.65,0.65,1.,0.95],Main),
        Post => g.cuboid([0.35,0.35,0.,0.65,0.65,1.],Main),
        Quarter => g.cuboid([0.,0.,0.,0.5,0.5,1.],Main),
        Panel => g.cuboid([0.,0.,0.,1.,0.08,1.],Main),
        Grille => {for n in 0..4 {let x=0.05+n as f32*0.3;g.cuboid([x,0.44,0.,x+0.05,0.56,1.],Main);}for z in [0.2,0.7] {g.cuboid([0.,0.44,z,1.,0.56,z+0.05],Main);}},
        Arch|HalfArch => {
            let end=if form==HalfArch {8}else{16};
            for n in 0..end {
                let a=n as f32*std::f32::consts::PI/16.;let b=(n+1) as f32*std::f32::consts::PI/16.;
                let p=|r:f32,t:f32,y:f32|[0.5+r*t.cos(),y,0.5+r*t.sin()];
                let points=[p(0.5,a,0.25),p(0.5,b,0.25),p(0.3,b,0.25),p(0.3,a,0.25),p(0.5,a,0.75),p(0.5,b,0.75),p(0.3,b,0.75),p(0.3,a,0.75)];
                for f in [[0,1,2,3],[4,7,6,5],[0,4,5,1],[3,2,6,7]] {g.face(f.map(|i|points[i]),Main);}
                g.boxes.push(bounds(&points));
            }
            g.cuboid([0.8,0.25,0.,1.,0.75,0.5],Main);
            if form==Arch {g.cuboid([0.,0.25,0.,0.2,0.75,0.5],Main);}
        }
        Arc15Small=>g.arc(0.5,15.),Arc30Small=>g.arc(0.5,30.),Arc45Small=>g.arc(0.5,45.),Arc90Small=>g.arc(0.5,90.),
        Arc15Medium=>g.arc(0.75,15.),Arc30Medium=>g.arc(0.75,30.),Arc45Medium=>g.arc(0.75,45.),Arc90Medium=>g.arc(0.75,90.),
        Arc15Large=>g.arc(1.,15.),Arc30Large=>g.arc(1.,30.),Arc45Large=>g.arc(1.,45.),Arc90Large=>g.arc(1.,90.),
        Dome=>g.dome(360.,0.5,0.5),DomeHalf=>g.dome(180.,0.5,0.5),DomeQuarter=>g.dome(90.,1.,0.),
        Door|DoorTop|DoorOpen|DoorTopOpen|SlidingDoor|SlidingDoorTop|SlidingDoorOpen|SlidingDoorTopOpen|FoldingDoor|FoldingDoorTop|FoldingDoorOpen|FoldingDoorTopOpen => {
            let open=matches!(form,DoorOpen|DoorTopOpen|SlidingDoorOpen|SlidingDoorTopOpen|FoldingDoorOpen|FoldingDoorTopOpen);
            let sliding=matches!(form,SlidingDoor|SlidingDoorTop|SlidingDoorOpen|SlidingDoorTopOpen);
            let folding=matches!(form,FoldingDoor|FoldingDoorTop|FoldingDoorOpen|FoldingDoorTopOpen);
            let (width,depth): (f32,f32)=if !open {(1.,0.1)}else if sliding {(0.16,0.1)}else if folding {(0.22,0.32)}else{(0.1,1.)};
            // Wood/metal frame and an inset glass panel; both halves toggle together.
            g.cuboid([0.,0.,0.,width.min(0.08),depth,1.],Main);
            g.cuboid([(width-0.08).max(0.),0.,0.,width,depth,1.],Main);
            g.cuboid([0.,0.,0.,width,depth,0.09],Main);
            g.cuboid([0.,0.,0.91,width,depth,1.],Main);
            if width>0.16 {g.cuboid([0.08,depth*0.35,0.09,width-0.08,depth*0.65,0.91],Surface::Glass);}
            else {g.cuboid([0.,0.,0.09,width,depth,0.91],Main);}
            let handle_y=if depth>0.9 {depth-0.06}else{depth};
            g.cuboid([(width-0.12).max(0.),handle_y,0.43,(width-0.04).max(0.04),(handle_y+0.05).min(1.),0.55],Surface::Metal);
        }
        Chair => {
            legs(&mut g,0.5);g.cuboid([0.08,0.08,0.46,0.92,0.92,0.57],Surface::Fabric);
            g.cuboid([0.08,0.80,0.57,0.92,0.92,1.],Main);
        }
        Table|GlassTable => {legs(&mut g,0.78);g.cuboid([0.,0.,0.78,1.,1.,0.88],if form==GlassTable {Surface::Glass}else{Main});},
        Sofa|SofaCorner => {
            g.cuboid([0.,0.03,0.12,1.,0.94,0.38],Main);g.cuboid([0.,0.06,0.38,1.,0.78,0.55],Surface::Fabric);
            g.cuboid([0.,0.78,0.38,1.,0.96,0.94],Surface::Fabric);
            if joined&1==0||form==SofaCorner {g.cuboid([0.,0.03,0.35,0.13,0.78,0.78],Surface::Fabric);}
            if joined&2==0 {g.cuboid([0.87,0.03,0.35,1.,0.78,0.78],Surface::Fabric);}
        }
        Bed => {
            // Two blocks long (a player lies on it): the foot half is this cell, the head
            // half with the pillow and the headboard reaches into the next one.
            for x in [0.04,0.84] {g.cuboid([x,0.04,0.,x+0.12,0.16,0.28],Main);g.visual([x,1.84,0.,x+0.12,1.96,0.28],Main);}
            g.cuboid([0.,0.,0.25,1.,1.,0.42],Main);g.visual([0.,1.,0.25,1.,2.,0.42],Main);
            g.cuboid([0.02,0.02,0.42,0.98,1.,0.58],Surface::Fabric);g.visual([0.02,1.,0.42,0.98,1.45,0.58],Surface::Fabric);
            g.visual([0.1,1.50,0.42,0.9,1.90,0.64],Surface::Ceramic);
            g.visual([0.,1.94,0.25,1.,2.,0.95],Main);
        },
        Wardrobe|KitchenBase|Nightstand => {
            let h=if form==Wardrobe{1.}else if form==KitchenBase{0.88}else{0.62};
            g.cuboid([0.02,0.12,0.,0.98,0.96,h],Main);
            if form==Wardrobe {
                // As tall as a player: the upper half is drawn above the cell.
                g.visual([0.02,0.12,1.,0.98,0.96,1.9],Main);
                g.visual([0.49,0.10,0.06,0.51,0.12,1.84],Surface::Metal);
                for x in [0.42,0.55] {g.visual([x,0.06,0.85,x+0.03,0.12,1.15],Surface::Metal);}
            } else {
                for x in [0.44,0.54] {g.cuboid([x,0.06,h*0.45,x+0.03,0.12,h*0.65],Surface::Metal);}
            }
        }
        Shelf => {
            for x in [0.02,0.91] {g.cuboid([x,0.15,0.,x+0.07,0.9,1.],Main);}
            for z in [0.,0.31,0.64,0.94] {g.cuboid([0.02,0.15,z,0.98,0.9,z+0.06],Main);}
        }
        Counter|Sink => {
            g.cuboid([0.,0.08,0.,1.,0.92,0.78],Main);
            if form==Counter {g.cuboid([0.,0.,0.78,1.,1.,0.9],Surface::Ceramic);}
            else {
                for b in [[0.,0.,0.78,0.2,1.,0.9],[0.8,0.,0.78,1.,1.,0.9],[0.2,0.,0.78,0.8,0.2,0.9],[0.2,0.8,0.78,0.8,1.,0.9],[0.2,0.2,0.72,0.8,0.8,0.75]] {g.cuboid(b,Surface::Ceramic);}
                g.cuboid([0.46,0.84,0.9,0.54,0.92,1.],Surface::Metal);
            }
        }
        FloorLamp => {g.cuboid([0.25,0.25,0.,0.75,0.75,0.08],Surface::Metal);g.cuboid([0.47,0.47,0.08,0.53,0.53,1.],Surface::Metal);g.visual([0.47,0.47,1.,0.53,0.53,1.3],Surface::Metal);g.visual([0.2,0.2,1.3,0.8,0.8,1.65],Surface::Light);},
        Curtain => {for n in 0..8 {let x=n as f32/8.;let y=if n%2==0{0.08}else{0.16};g.cuboid([x,y,0.,x+0.125,y+0.04,1.],Surface::Fabric);}},
        Rug=>g.cuboid([0.,0.,0.,1.,1.,0.025],Surface::Fabric),
        Mirror=>{g.cuboid([0.05,0.04,0.05,0.95,0.12,0.95],Main);g.cuboid([0.11,0.01,0.11,0.89,0.04,0.89],Surface::Mirror);},
        FlowerPot=>{g.cuboid([0.28,0.28,0.,0.72,0.72,0.37],Surface::Ceramic);g.cuboid([0.23,0.23,0.3,0.77,0.77,0.40],Surface::Ceramic);g.cuboid([0.46,0.46,0.4,0.54,0.54,0.85],Surface::Plant);g.cuboid([0.3,0.3,0.7,0.7,0.7,0.95],Surface::Plant);},
    }
    g
}
fn legs(g:&mut Geometry,h:f32) {for x in [0.08,0.8] {for y in [0.08,0.8] {g.cuboid([x,y,0.,x+0.12,y+0.12,h],Surface::Main);}}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_models_have_bounded_geometry_and_collision() {
        for entry in crate::construction::ENTRIES {
            for rotation in 0..4 {
                let g=model(entry.form,rotation,0);
                assert!(!g.faces.is_empty()&&!g.boxes.is_empty(),"{}",entry.name);
                for b in &g.boxes {for a in 0..3 {assert!(b[a]>=-1e-5&&b[a+3]<=1.00001&&b[a]<b[a+3],"{} {:?}",entry.name,b);}}
                // Big furniture (bed, wardrobe, floor lamp) is drawn up to one cell beyond its own.
                let big=matches!(entry.form,Form::Bed|Form::Wardrobe|Form::FloorLamp);
                let (lo,hi)=if big {(-1.00001,2.00001)} else {(-1e-5,1.00001)};
                for face in &g.faces {for p in face.points {assert!(p.iter().all(|v|v.is_finite()&&*v>=lo&&*v<=hi),"{} {:?}",entry.name,p);}}
            }
        }
    }
    #[test]
    fn curved_glass_is_a_shell_and_sofas_join() {
        let arc=model(Form::Arc90Large,0,0);
        assert!(arc.boxes.len()>=12);
        assert!(!arc.boxes.iter().any(|b| b[0]<0.2&&b[1]<0.2));
        assert!(model(Form::Sofa,0,3).faces.len()<model(Form::Sofa,0,0).faces.len());
    }
}

#[cfg(test)]
mod preview {
    /// Writes every form's faces for a contact sheet.
    #[test]
    #[ignore]
    fn dump_forms() {
        let mut s = String::new();
        for (i, (form, name)) in super::super::FORMS.iter().enumerate() {
            let g = super::model(*form, 0, 0);
            s += &format!("# {i} {name}\n");
            for f in &g.faces {
                let p: Vec<String> = f.points.iter().map(|p| format!("{} {} {}", p[0], p[1], p[2])).collect();
                s += &format!("{} {}\n", p.join(" "), f.surface as u8);
            }
        }
        std::fs::write(std::env::var("FORMS_OUT").unwrap(), s).unwrap();
    }
}
