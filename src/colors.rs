use bevy::prelude::Color;

use crate::attributes::ItemRarity;

pub const RED: Color = Color::srgba(145. / 255., 54. / 255., 54. / 255., 1.);
pub const DMG_NUM_RED: Color = Color::srgba(179. / 255., 61. / 255., 61. / 255., 1.);
pub const DMG_NUM_GREEN: Color = Color::srgba(185. / 255., 185. / 255., 110. / 255., 1.);
pub const DMG_NUM_PURPLE: Color = Color::srgba(81. / 255., 65. / 255., 104. / 255., 1.);
pub const DMG_NUM_YELLOW: Color = Color::srgba(231. / 255., 193. / 255., 111. / 255., 1.);
pub const DMG_NUM_ORANGE: Color = Color::srgba(255. / 255., 140. / 255., 50. / 255., 1.); // Overcrit color
pub const _GOLD: Color = Color::srgba(201. / 255., 114. / 255., 69. / 255., 1.);
pub const ORANGE: Color = Color::srgba(201. / 255., 109. / 255., 69. / 255., 1.);
pub const LIGHT_RED: Color = Color::srgba(202. / 255., 53. / 255., 55. / 255., 1.);
pub const LIGHT_GREY: Color = Color::srgba(160. / 255., 155. / 255., 131. / 255., 1.);
pub const SHRINE_GREEN: Color = Color::srgba(0. / 255., 255. / 255., 15. / 255., 1.);
pub const LIGHT_GREEN: Color = Color::srgba(163. / 255., 182. / 255., 69. / 255., 1.);
pub const LIGHT_BLUE: Color = Color::srgba(98. / 255., 153. / 255., 178. / 255., 1.);
pub const LIGHT_BROWN: Color = Color::srgba(168. / 255., 112. / 255., 71. / 255., 1.);
pub const _UI_GRASS_GREEN: Color = Color::srgba(154. / 255., 169. / 255., 76. / 255., 1.);
pub const GREY: Color = Color::srgba(123. / 255., 119. / 255., 101. / 255., 1.);
pub const UNCOMMON_GREEN: Color = Color::srgba(51. / 255., 116. / 255., 60. / 255., 1.);
pub const DARK_GREEN: Color = Color::srgba(31. / 255., 85. / 255., 49. / 255., 1.);
pub const _BLACK_GREEN: Color = Color::srgba(45. / 255., 61. / 255., 56. / 255., 1.);
pub const DARK_BROWN: Color = Color::srgba(101. / 255., 58. / 255., 12. / 255., 1.);
pub const DARK_WOOD_BROWN: Color = Color::srgba(59. / 255., 47. / 255., 28. / 255., 1.);
pub const BLACK: Color = Color::srgba(28. / 255., 48. / 255., 41. / 255., 1.);
pub const NIGHT: Color = Color::srgba(28. / 255., 48. / 255., 41. / 255., 0.55);
// pub const BLUE: Color = Color::srgba(61. / 255., 112. / 255., 133. / 255., 1.);
pub const BLUE: Color = Color::srgba(43. / 255., 119. / 255., 125. / 255., 1.);
pub const SHIELD_BLUE: Color = Color::srgba(119. / 255., 249. / 255., 253. / 255., 0.5);
pub const YELLOW: Color = Color::srgba(237. / 255., 182. / 255., 54. / 255., 1.);
pub const YELLOW_2: Color = Color::srgba(223. / 255., 178. / 255., 91. / 255., 1.);
pub const WHITE: Color = Color::srgba(226. / 255., 212. / 255., 177. / 255., 1.);
pub const PINK: Color = Color::srgba(255. / 255., 136. / 255., 169. / 255., 1.);
pub const LEVEL_BLUE: Color = Color::srgba(77. / 255., 215. / 255., 225. / 255., 1.);
pub const LEVEL_DARK_BLUE: Color = Color::srgba(67. / 255., 86. / 255., 122. / 255., 1.);
/// Dark blue used for the third player map marker (`#0064c8`).
pub const MAP_MARKER_BLUE: Color = Color::srgba(0. / 255., 100. / 255., 200. / 255., 1.);
pub const TOOLTIP_BLACK: Color = Color::srgba(39. / 255., 39. / 255., 39. / 255., 1.);
pub const TOOLTIP_BLACK_2: Color = Color::srgba(68. / 255., 68. / 255., 68. / 255., 1.);

pub const DESERT_TILE: Color = Color::srgba(255. / 255., 228. / 255., 136. / 255., 1.);
pub const DESERT_WATER: Color = Color::srgba(255. / 255., 187. / 255., 85. / 255., 1.);
pub const SNOW_TILE: Color = Color::srgba(147. / 255., 198. / 255., 202. / 255., 1.);
pub const SNOW_WATER: Color = Color::srgba(19. / 255., 67. / 255., 127. / 255., 1.);
pub const SNOW_DARK: Color = Color::srgba(73. / 255., 105. / 255., 117. / 255., 1.);
pub const SNOW_BLUE: Color = Color::srgba(81. / 255., 176. / 255., 213. / 255., 1.);
pub const SNOW_GREEN: Color = Color::srgba(86. / 255., 128. / 255., 66. / 255., 1.);

pub const STATS_TITLE: Color = Color::srgba(199. / 255., 235. / 255., 243. / 255., 1.);
pub const HOTBAR_TITLE: Color = Color::srgba(255. / 255., 191. / 255., 153. / 255., 1.);
pub const EQUIP_TITLE: Color = Color::srgba(72. / 255., 40. / 255., 26. / 255., 1.);
pub const CRAFT_BUTTON_TEXT: Color = Color::srgba(225. / 255., 224. / 255., 164. / 255., 1.);

pub const COMMON_TOOLTIP_TITLE: Color = Color::srgba(225. / 255., 222. / 255., 231. / 255., 1.);
pub const UNCOMMON_TOOLTIP_TITLE: Color = Color::srgba(186. / 255., 243. / 255., 234. / 255., 1.);
pub const RARE_TOOLTIP_TITLE: Color = Color::srgba(234. / 255., 140. / 255., 245. / 255., 1.);
pub const LEGENDARY_TOOLTIP_TITLE: Color = Color::srgba(233. / 255., 149. / 255., 7. / 255., 1.);

pub fn overwrite_alpha(color: Color, alpha: f32) -> Color {
    let c = color.to_srgba();
    Color::srgba(c.red, c.green, c.blue, alpha)
}
