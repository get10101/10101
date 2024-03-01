import 'package:carousel_slider/carousel_slider.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/features/welcome/payout_chart.dart';
import 'package:get_10101/features/welcome/seed_import_screen.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:go_router/go_router.dart';

final themeMode = ValueNotifier(2);

class CarouselItem {
  final String title;
  final String description;
  final Widget imageWidget;

  CarouselItem(
    this.title,
    this.description,
    this.imageWidget,
  );
}

final List<CarouselItem> carouselItems = [
  CarouselItem(
      "Welcome, Satoshi",
      "10101 uses Discreet Log Contracts (DLCs) to ensure that every trade is collateralized and fully self-custodial. \n\nYour first trade will open a DLC channel with on-chain funds. Every subsequent trade will happen off-chain until your channel is closed.",
      Image.asset("assets/carousel_1.png")),
  CarouselItem(
      "Perpetual Futures",
      "In 10101 you can trade Perpetual Futures (CFDs) without counterparty risk. \n \nThe max amount you put at risk will depend on how much you lock up. At the same time, the amount you can win will depend on the amount your channel counterparty will lock up.",
      const PnlLineChart()),
  CarouselItem(
      "It's the future",
      "Once you have an open channel, you can trade instantly and without transaction fees. \n In the future you will be able to extend or reduce the channel size (splice-in/splice-out). ",
      Image.asset("assets/carousel_3.png")),
];

Widget carouselItemWidget(BuildContext context, CarouselItem item) {
  return Column(
    children: [
      Expanded(
        child: Column(
          children: [
            Expanded(
              child: FractionallySizedBox(heightFactor: 0.8, child: item.imageWidget),
            ),
            Padding(
              padding: const EdgeInsets.only(left: 20, right: 20, top: 0),
              child: Text(
                item.title,
                style: const TextStyle(fontSize: 30, fontWeight: FontWeight.bold),
                textAlign: TextAlign.left,
              ),
            ),
            Padding(
              padding: const EdgeInsets.only(left: 20, right: 20, top: 10),
              child: Text(
                item.description,
                style: const TextStyle(fontSize: 18, color: Colors.black54, letterSpacing: 0.1),
                textAlign: TextAlign.justify,
              ),
            )
          ],
        ),
      ),
    ],
  );
}

class Onboarding extends StatefulWidget {
  static const route = "/on-boarding";
  static const label = "Welcome";

  const Onboarding({Key? key}) : super(key: key);

  @override
  State<StatefulWidget> createState() {
    return _Onboarding();
  }
}

class _Onboarding extends State<Onboarding> {
  int _current = 0;
  final CarouselController _controller = CarouselController();

  @override
  Widget build(BuildContext context) {
    List<Widget> carouselItemWidgetLayers = [
      carouselItemWidget(context, carouselItems[0]),
      carouselItemWidget(context, carouselItems[1]),
      carouselItemWidget(context, carouselItems[2]),
    ];

    return Scaffold(
        backgroundColor: Colors.white,
        body: SafeArea(
            child: Container(
          color: Colors.white,
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              SizedBox(
                height: MediaQuery.of(context).size.height * 0.70,
                child: CarouselSlider(
                  items: carouselItemWidgetLayers,
                  carouselController: _controller,
                  options: CarouselOptions(
                      viewportFraction: 1.0,
                      scrollDirection: Axis.horizontal,
                      autoPlay: false,
                      enlargeCenterPage: true,
                      aspectRatio: 15 / 22,
                      padEnds: true,
                      enableInfiniteScroll: false,
                      onPageChanged: (index, reason) {
                        setState(() {
                          _current = index;
                        });
                      }),
                ),
              ),
              Expanded(
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: carouselItemWidgetLayers.asMap().entries.map((entry) {
                    return GestureDetector(
                      onTap: () => _controller.animateToPage(entry.key),
                      child: Container(
                        width: 8.0,
                        height: 8.0,
                        margin: const EdgeInsets.symmetric(vertical: 8.0, horizontal: 4.0),
                        decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            color: (Theme.of(context).brightness == Brightness.dark
                                    ? Colors.white
                                    : Colors.black)
                                .withOpacity(_current == entry.key ? 0.6 : 0.2)),
                      ),
                    );
                  }).toList(),
                ),
              ),
              const SizedBox(height: 10),
              Column(children: [
                SizedBox(
                  width: 250,
                  child: ElevatedButton(
                      onPressed: () => GoRouter.of(context).go(WelcomeScreen.route),
                      style: ButtonStyle(
                        padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                        backgroundColor: MaterialStateProperty.all<Color>(tenTenOnePurple),
                        shape: MaterialStateProperty.all<RoundedRectangleBorder>(
                          RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(40.0),
                            side: const BorderSide(color: tenTenOnePurple),
                          ),
                        ),
                      ),
                      child: const Wrap(
                        children: <Widget>[
                          Text(
                            "Create new wallet",
                            style: TextStyle(fontSize: 18, color: Colors.white),
                          ),
                        ],
                      )),
                ),
                const SizedBox(height: 5),
                SizedBox(
                  width: 250,
                  child: TextButton(
                    onPressed: () => GoRouter.of(context).go(SeedPhraseImporter.route),
                    style: ButtonStyle(
                      padding: MaterialStateProperty.all<EdgeInsets>(const EdgeInsets.all(15)),
                      backgroundColor: MaterialStateProperty.all<Color>(Colors.white),
                    ),
                    child: const Wrap(
                      children: <Widget>[
                        Text(
                          "Restore from backup",
                          style: TextStyle(
                            fontSize: 18,
                            color: Colors.black,
                            decoration: TextDecoration.underline,
                          ),
                        ),
                      ],
                    ),
                  ),
                )
              ]),
            ],
          ),
        )));
  }
}
