import 'package:flutter/material.dart';
import 'package:get_10101/features/trade/trade_theme.dart';

class TradeTabs extends StatelessWidget {
  const TradeTabs(
      {required this.tabs,
      required this.tabBarViewChildren,
      this.tabLabelPadding = const EdgeInsets.symmetric(horizontal: 3.0, vertical: 3.0),
      this.tabBarPadding = const EdgeInsets.symmetric(vertical: 10.0),
      this.tabSpacing = 5.0,
      this.tabBorderRadius = const BorderRadius.all(Radius.circular(50)),
      this.selectedIndex = 0,
      this.tabLabelTextOffset = 30,
      this.topRightWidget,
      super.key});

  final List<String> tabs;
  final List<Widget> tabBarViewChildren;

  /// Offset that is added to the label based on the longest tab label text.
  /// By changing this you can define the tab's size in relation to the longest label text.
  final double tabLabelTextOffset;
  final EdgeInsets tabLabelPadding;
  final EdgeInsets tabBarPadding;
  final double tabSpacing;
  final BorderRadius tabBorderRadius;
  final int selectedIndex;

  final Widget? topRightWidget;

  @override
  Widget build(BuildContext context) {
    assert(tabs.length == tabBarViewChildren.length);

    TradeTheme tradeTheme = Theme.of(context).extension<TradeTheme>()!;

    // in case we ever want to change the text style of the tab label
    const TextStyle textStyle = TextStyle();
    double tabWidth = _tabWidth(tabs, textStyle);

    return DefaultTabController(
        initialIndex: selectedIndex,
        length: tabs.length,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Container(
              padding: tabBarPadding,
              // Set an explicit width
              // width: appBarWidth,
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                crossAxisAlignment: CrossAxisAlignment.center,
                children: [
                  TabBar(
                      // This stops expanding the tab-bar to the right.
                      // This lets us align the tab bar to wherever we want, because it's size is limited to the tabs that it contains.
                      isScrollable: true,

                      // suppress overlay color (used for e.g. mouse-over) because it looks weird in this tab setup
                      overlayColor: MaterialStateColor.resolveWith((states) => Colors.transparent),

                      // We don't have a bottom indicator; set to 0 to avoid it taking space when being unseleced.
                      indicatorWeight: 0,

                      // Defines the distance the tabs are apart; unfortunately there is no way to achieve this without padding; using a marin on the container breaks the dependency between the selected and unselected tab.
                      // Padding for indicator and label have to be aligned otherwise we again break the alignment of selected and unselected tab.
                      // We only use padding to right, otherwise the first tab is not aligned on the left.
                      indicatorPadding: EdgeInsets.only(right: tabSpacing),
                      labelPadding: EdgeInsets.only(right: tabSpacing),

                      // expands the pill to be the size of the tab to align with the padding
                      indicatorSize: TabBarIndicatorSize.tab,
                      unselectedLabelColor: tradeTheme.tabColor,
                      indicator:
                          BoxDecoration(borderRadius: tabBorderRadius, color: tradeTheme.tabColor),
                      tabs: tabs
                          .map((label) => Container(
                                width: tabWidth,
                                padding: tabLabelPadding,
                                decoration: BoxDecoration(
                                    borderRadius: tabBorderRadius,
                                    border: Border.all(color: tradeTheme.tabColor, width: 1)),
                                child: Align(
                                  alignment: Alignment.center,
                                  child: Text(
                                    label,
                                    style: textStyle,
                                  ),
                                ),
                              ))
                          .toList()),
                  if (topRightWidget != null) topRightWidget!,
                ],
              ),
            ),
            Expanded(
              child: TabBarView(
                children: tabBarViewChildren,
              ),
            )
          ],
        ));
  }

  /// Calculates the tab width based on the text label of the tab
  double _tabWidth(List<String> tabs, TextStyle textStyle) {
    double maxTextWidth = 0;

    for (var label in tabs) {
      double width = _textSize(label, textStyle).width;
      if (width > maxTextWidth) {
        maxTextWidth = width;
      }
    }

    return maxTextWidth + tabLabelTextOffset;
  }

  Size _textSize(String text, TextStyle style) {
    final TextPainter textPainter = TextPainter(
        text: TextSpan(text: text, style: style), maxLines: 1, textDirection: TextDirection.ltr)
      ..layout(minWidth: 0, maxWidth: double.infinity);
    return textPainter.size;
  }
}
