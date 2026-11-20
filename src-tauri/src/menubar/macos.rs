//! 메뉴바 아이콘을 AppKit(macOS 화면 프레임워크)으로 직접 그린다. macOS 전용.
//!
//! ## 왜 직접 그리나
//! 메뉴바 글자색은 라이트/다크 모드뿐 아니라 배경화면 밝기에 따라서도 바뀐다.
//! 이미지를 미리 픽셀(RGBA)로 만들어 넘기면 그 순간의 색으로 굳어 버려서,
//! 나중에 메뉴바 밝기가 바뀌면 글자가 안 보이거나 색이 틀어진다.
//! 그래서 NSImage 에 "그리는 방법"(drawingHandler)을 넘겨 두고 캐시를 끈다.
//! 그러면 macOS 가 화면에 그릴 때마다 그 순간의 메뉴바 밝기로 `labelColor` 를 계산한다.
//!
//! ## objc2 메모
//! - `Retained<T>`: Objective-C 객체를 가리키는 Rust 포인터. 다 쓰면 자동으로 해제된다.
//! - `unsafe { ... }`: 컴파일러가 안전성을 증명할 수 없는 호출이라는 표시. 대부분 Objective-C 호출이다.
//! - `MainThreadMarker`: "지금 메인 스레드에 있다"는 증표. 화면 객체는 메인 스레드에서만 다룬다.
//! - 좌표: AppKit 은 왼쪽 **아래**가 (0, 0) 이고 위로 갈수록 y 가 커진다.

use std::path::Path;

use block2::{RcBlock, StackBlock};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{AllocAnyThread, MainThreadMarker};
use objc2_app_kit::{
    NSAppearance, NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSBezierPath,
    NSBitmapImageFileType, NSBitmapImageRep, NSCellImagePosition, NSColor, NSCompositingOperation,
    NSDeviceRGBColorSpace, NSFont, NSFontAttributeName, NSFontWeightMedium,
    NSForegroundColorAttributeName, NSGraphicsContext, NSImage, NSImageCacheMode, NSStatusBar,
    NSStatusItem, NSStringDrawing,
};
use objc2_foundation::{NSDictionary, NSPoint, NSRect, NSSize, NSString};

use super::MenubarModel;
use crate::model::LabelKind;
use crate::settings::MenubarStyle;

// 크기 (단위: pt). 메뉴바 높이는 보통 22pt 다.
const FONT_SIZE: f64 = 12.0;
/// 라벨과 막대(또는 숫자) 사이
const LABEL_GAP: f64 = 3.0;
/// 계정과 계정 사이
const ITEM_GAP: f64 = 9.0;
const BAR_WIDTH: f64 = 30.0;
const BAR_HEIGHT: f64 = 6.0;
/// 시간 눈금의 굵기
const MARKER_WIDTH: f64 = 1.5;
/// 시간 눈금이 막대 위아래로 튀어나오는 길이
const MARKER_OVERHANG: f64 = 2.5;
/// 눈금 양옆을 비워 두는 폭. 눈금이 채워진 부분 위에 있어도 잘 보이게 한다.
const MARKER_GAP: f64 = 1.0;
const DOT_SIZE: f64 = 7.0;
/// 이미지 양 끝 여백
const EDGE_PADDING: f64 = 1.0;
/// 상태 항목(메뉴바 칸) 안쪽 좌우 여백
const STATUS_ITEM_PADDING: f64 = 5.0;

/// 그릴 조각 하나. 위치(x)는 이미지를 만들기 전에 미리 계산해 둔다.
enum Piece {
    Text { x: f64, text: String, warn: bool },
    Dot { x: f64, rgb: (f64, f64, f64) },
    Bar { x: f64, used: f64, elapsed: f64, warn: bool },
}

/// 메뉴바 항목(상태 항목)의 이미지를 새로 그려서 바꿔 끼운다. 메인 스레드에서만 부를 것.
pub fn apply(status_item: &NSStatusItem, model: &MenubarModel, mtm: MainThreadMarker) {
    let image = build_image(model);
    // 칸 너비를 이미지에 맞춰 직접 정한다. (자동 크기 조절은 반영이 늦을 때가 있다)
    status_item.setLength(image.size().width + STATUS_ITEM_PADDING * 2.0);
    if let Some(button) = status_item.button(mtm) {
        button.setImagePosition(NSCellImagePosition::ImageOnly);
        button.setImage(Some(&image));
    }
}

/// 메뉴바 이미지를 만든다. 실제 그리기는 macOS 가 화면에 그릴 때마다 실행된다.
pub fn build_image(model: &MenubarModel) -> Retained<NSImage> {
    let font = menubar_font();
    let (pieces, width) = layout(model, &font);
    let height = NSStatusBar::systemStatusBar().thickness();

    // `move` 클로저: pieces 와 font 의 소유권을 그리기 함수 안으로 옮긴다.
    // 이미지가 살아 있는 동안 macOS 가 이 함수를 여러 번 부를 수 있다.
    let draw_handler = RcBlock::new(move |rect: NSRect| -> Bool {
        draw(&pieces, &font, rect.size.height);
        Bool::YES
    });

    let image = NSImage::imageWithSize_flipped_drawingHandler(
        NSSize::new(width, height),
        false,
        &draw_handler,
    );
    // 캐시를 끄면 메뉴바 밝기가 바뀔 때마다 다시 그려져서 색이 항상 맞는다.
    image.setCacheMode(NSImageCacheMode::Never);
    // 색이 하나도 없으면 템플릿 이미지로 표시해 macOS 가 직접 색을 입히게 한다.
    image.setTemplate(model.is_monochrome());
    image
}

fn menubar_font() -> Retained<NSFont> {
    // 숫자 폭이 일정한 시스템 글꼴 (숫자가 바뀌어도 너비가 흔들리지 않는다)
    NSFont::monospacedDigitSystemFontOfSize_weight(FONT_SIZE, unsafe { NSFontWeightMedium })
}

/// 글자 그리기에 쓸 속성(글꼴, 색) 묶음.
fn text_attributes(font: &NSFont, color: &NSColor) -> Retained<NSDictionary<NSString, AnyObject>> {
    let keys: [&NSString; 2] = unsafe { [NSFontAttributeName, NSForegroundColorAttributeName] };
    let values: [&AnyObject; 2] = [font, color];
    NSDictionary::from_slices(&keys, &values)
}

fn text_width(text: &str, font: &NSFont) -> f64 {
    let attributes = text_attributes(font, &NSColor::labelColor());
    unsafe { NSString::from_str(text).sizeWithAttributes(Some(&attributes)) }.width
}

/// 각 조각의 x 위치와 전체 너비를 계산한다.
fn layout(model: &MenubarModel, font: &NSFont) -> (Vec<Piece>, f64) {
    let mut pieces = Vec::new();
    let mut x = EDGE_PADDING;

    if model.items.is_empty() {
        // 아직 계정이 없을 때
        let text = "AI".to_string();
        let width = text_width(&text, font);
        pieces.push(Piece::Text { x, text, warn: false });
        return (pieces, x + width + EDGE_PADDING);
    }

    for (index, item) in model.items.iter().enumerate() {
        if index > 0 {
            x += ITEM_GAP;
        }

        // 1) 라벨: 별칭·이모지는 글자로, 색 점은 동그라미로
        match item.label_kind {
            LabelKind::Dot => {
                pieces.push(Piece::Dot { x, rgb: item.color });
                x += DOT_SIZE;
            }
            LabelKind::Alias | LabelKind::Emoji => {
                let width = text_width(&item.label_text, font);
                pieces.push(Piece::Text { x, text: item.label_text.clone(), warn: false });
                x += width;
            }
        }
        x += LABEL_GAP;

        // 2) 막대 또는 숫자
        match model.style {
            MenubarStyle::Bars => {
                pieces.push(Piece::Bar {
                    x,
                    used: item.used_percent,
                    elapsed: item.elapsed_percent,
                    warn: item.warn,
                });
                x += BAR_WIDTH;
            }
            MenubarStyle::Numbers => {
                let text = model.number_text(item);
                let width = text_width(&text, font);
                pieces.push(Piece::Text { x, text, warn: item.warn });
                x += width;
            }
        }
    }

    (pieces, x + EDGE_PADDING)
}

/// 실제 그리기. macOS 가 이미지를 화면에 그릴 때 불린다.
fn draw(pieces: &[Piece], font: &NSFont, height: f64) {
    // `labelColor` 는 "지금 그리는 곳"의 밝기에 맞는 글자색으로 바뀌는 동적 색이다.
    let normal = NSColor::labelColor();
    let red = NSColor::systemRedColor();
    let center_y = height / 2.0;

    for piece in pieces {
        match piece {
            Piece::Text { x, text, warn } => {
                let color = if *warn { &red } else { &normal };
                let attributes = text_attributes(font, color);
                let string = NSString::from_str(text);
                let size = unsafe { string.sizeWithAttributes(Some(&attributes)) };
                let origin = NSPoint::new(*x, (height - size.height) / 2.0);
                unsafe { string.drawAtPoint_withAttributes(origin, Some(&attributes)) };
            }
            Piece::Dot { x, rgb } => {
                NSColor::colorWithSRGBRed_green_blue_alpha(rgb.0, rgb.1, rgb.2, 1.0).setFill();
                let rect = NSRect::new(
                    NSPoint::new(*x, center_y - DOT_SIZE / 2.0),
                    NSSize::new(DOT_SIZE, DOT_SIZE),
                );
                NSBezierPath::bezierPathWithOvalInRect(rect).fill();
            }
            Piece::Bar { x, used, elapsed, warn } => {
                let fill_color = if *warn { &red } else { &normal };
                draw_bar(*x, center_y, *used, *elapsed, fill_color, &normal);
            }
        }
    }
}

/// 막대 하나: 흐린 바탕 → 사용량만큼 채움 → 시간 눈금.
fn draw_bar(x: f64, center_y: f64, used: f64, elapsed: f64, fill: &NSColor, base: &NSColor) {
    let Some(context) = NSGraphicsContext::currentContext() else {
        return;
    };
    let bar_rect = NSRect::new(
        NSPoint::new(x, center_y - BAR_HEIGHT / 2.0),
        NSSize::new(BAR_WIDTH, BAR_HEIGHT),
    );
    let radius = BAR_HEIGHT / 2.0;
    let bar_shape = NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(bar_rect, radius, radius);

    // 1) 바탕 (흐리게)
    base.colorWithAlphaComponent(0.3).setFill();
    bar_shape.fill();

    // 2) 사용량만큼 채우기. 둥근 막대 모양으로 잘라서(clip) 그리므로 1%도 자연스럽게 보인다.
    let fill_width = BAR_WIDTH * used.clamp(0.0, 100.0) / 100.0;
    if fill_width > 0.0 {
        context.saveGraphicsState();
        bar_shape.addClip();
        fill.setFill();
        NSBezierPath::bezierPathWithRect(NSRect::new(bar_rect.origin, NSSize::new(fill_width, BAR_HEIGHT)))
            .fill();
        context.restoreGraphicsState();
    }

    // 3) 시간 눈금: 양옆을 투명하게 지운 뒤 막대보다 길게 그린다.
    let marker_center = x + BAR_WIDTH * elapsed.clamp(0.0, 100.0) / 100.0;
    let marker_height = BAR_HEIGHT + MARKER_OVERHANG * 2.0;
    let marker_bottom = center_y - marker_height / 2.0;

    context.saveGraphicsState();
    context.setCompositingOperation(NSCompositingOperation::Clear);
    NSBezierPath::bezierPathWithRect(NSRect::new(
        NSPoint::new(marker_center - MARKER_WIDTH / 2.0 - MARKER_GAP, marker_bottom),
        NSSize::new(MARKER_WIDTH + MARKER_GAP * 2.0, marker_height),
    ))
    .fill();
    context.restoreGraphicsState();

    base.setFill();
    let marker_rect = NSRect::new(
        NSPoint::new(marker_center - MARKER_WIDTH / 2.0, marker_bottom),
        NSSize::new(MARKER_WIDTH, marker_height),
    );
    NSBezierPath::bezierPathWithRoundedRect_xRadius_yRadius(
        marker_rect,
        MARKER_WIDTH / 2.0,
        MARKER_WIDTH / 2.0,
    )
    .fill();
}

/// 개발용: 메뉴바 이미지를 라이트 또는 다크 배경 위에 그려 PNG 파일로 저장한다.
/// (`cargo run --example render_menubar` 에서 쓴다. 앱 동작과는 무관하다.)
pub fn render_preview_png(model: &MenubarModel, dark: bool, path: &Path) -> Result<(), String> {
    let image = build_image(model);
    let size = image.size();
    let scale = 2.0; // 레티나 해상도
    let padding = 6.0;
    let (width, height) = (size.width + padding * 2.0, size.height + padding * 2.0);

    let bitmap = unsafe {
        NSBitmapImageRep::initWithBitmapDataPlanes_pixelsWide_pixelsHigh_bitsPerSample_samplesPerPixel_hasAlpha_isPlanar_colorSpaceName_bytesPerRow_bitsPerPixel(
            NSBitmapImageRep::alloc(),
            std::ptr::null_mut(),
            (width * scale) as isize,
            (height * scale) as isize,
            8,
            4,
            true,
            false,
            NSDeviceRGBColorSpace,
            0,
            0,
        )
    }
    .ok_or("비트맵을 만들지 못했어요")?;
    bitmap.setSize(NSSize::new(width, height));

    let context = NSGraphicsContext::graphicsContextWithBitmapImageRep(&bitmap)
        .ok_or("그리기 영역을 만들지 못했어요")?;
    let appearance_name = unsafe {
        if dark { NSAppearanceNameDarkAqua } else { NSAppearanceNameAqua }
    };
    let appearance =
        NSAppearance::appearanceNamed(appearance_name).ok_or("화면 모드를 찾지 못했어요")?;

    let previous = NSGraphicsContext::currentContext();
    NSGraphicsContext::setCurrentContext(Some(&context));
    let image_ref: &NSImage = &image;
    let draw_block = StackBlock::new(move || {
        let (r, g, b) = if dark { (0.17, 0.17, 0.19) } else { (0.92, 0.92, 0.93) };
        NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, 1.0).setFill();
        NSBezierPath::bezierPathWithRect(NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height)))
            .fill();
        image_ref.drawInRect(NSRect::new(NSPoint::new(padding, padding), size));
    });
    appearance.performAsCurrentDrawingAppearance(&draw_block);
    context.flushGraphics();
    NSGraphicsContext::setCurrentContext(previous.as_deref());

    let png = unsafe {
        bitmap.representationUsingType_properties(NSBitmapImageFileType::PNG, &NSDictionary::new())
    }
    .ok_or("PNG 로 바꾸지 못했어요")?;
    std::fs::write(path, png.to_vec()).map_err(|error| error.to_string())
}
