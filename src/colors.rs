use bevy::prelude::Color;

pub const RED: Color = Color::rgba(145. / 255., 54. / 255., 54. / 255., 1.);
pub const DMG_NUM_RED: Color = Color::rgba(179. / 255., 61. / 255., 61. / 255., 1.);
pub const DMG_NUM_GREEN: Color = Color::rgba(185. / 255., 185. / 255., 110. / 255., 1.);
pub const DMG_NUM_PURPLE: Color = Color::rgba(81. / 255., 65. / 255., 104. / 255., 1.);
pub const DMG_NUM_YELLOW: Color = Color::rgba(231. / 255., 193. / 255., 111. / 255., 1.);
pub const GOLD: Color = Color::rgba(201. / 255., 114. / 255., 69. / 255., 1.);
pub const LIGHT_RED: Color = Color::rgba(202. / 255., 53. / 255., 55. / 255., 1.);
pub const LIGHT_GREY: Color = Color::rgba(160. / 255., 155. / 255., 131. / 255., 1.);
pub const LIGHT_GREEN: Color = Color::rgba(163. / 255., 182. / 255., 69. / 255., 1.);
pub const LIGHT_BLUE: Color = Color::rgba(98. / 255., 153. / 255., 178. / 255., 1.);
pub const LIGHT_BROWN: Color = Color::rgba(168. / 255., 112. / 255., 71. / 255., 1.);
pub const UI_GRASS_GREEN: Color = Color::rgba(154. / 255., 169. / 255., 76. / 255., 1.);
pub const GREY: Color = Color::rgba(123. / 255., 119. / 255., 101. / 255., 1.);
pub const DARK_GREEN: Color = Color::rgba(31. / 255., 85. / 255., 49. / 255., 1.);
pub const _BLACK_GREEN: Color = Color::rgba(45. / 255., 61. / 255., 56. / 255., 1.);
pub const DARK_BROWN: Color = Color::rgba(101. / 255., 58. / 255., 12. / 255., 1.);
pub const BLACK: Color = Color::rgba(28. / 255., 48. / 255., 41. / 255., 1.);
pub const NIGHT: Color = Color::rgba(28. / 255., 48. / 255., 41. / 255., 0.55);
// pub const BLUE: Color = Color::rgba(61. / 255., 112. / 255., 133. / 255., 1.);
pub const BLUE: Color = Color::rgba(43. / 255., 119. / 255., 125. / 255., 1.);
pub const YELLOW: Color = Color::rgba(237. / 255., 182. / 255., 54. / 255., 1.);
pub const YELLOW_2: Color = Color::rgba(223. / 255., 178. / 255., 91. / 255., 1.);
pub const WHITE: Color = Color::rgba(226. / 255., 212. / 255., 177. / 255., 1.);
pub const PINK: Color = Color::rgba(255. / 255., 136. / 255., 169. / 255., 1.);

pub fn overwrite_alpha(color: Color, alpha: f32) -> Color {
    Color::rgba(color.r(), color.g(), color.b(), alpha)
}
