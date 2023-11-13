import 'package:carousel_slider/carousel_slider.dart';
import 'package:flutter/material.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/global_keys.dart';
import 'package:get_10101/common/scrollable_safe_area.dart';
import 'package:get_10101/common/snack_bar.dart';
import 'package:get_10101/features/wallet/wallet_screen.dart';
import 'package:get_10101/features/welcome/seed_import_screen.dart';
import 'package:get_10101/features/welcome/welcome_screen.dart';
import 'package:get_10101/ffi.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:get_10101/util/file.dart';
import 'package:get_10101/util/preferences.dart';
import 'package:go_router/go_router.dart';

final themeMode = ValueNotifier(2);

class CarouselItem {
  final String title;
  final String description;
  final String imagePath;

  CarouselItem(this.title, this.description, this.imagePath);
}

final List<CarouselItem> caruselItems = [
  CarouselItem("Your keys, your control", "Stay in control of your funds at all time.",
      "assets/carousel_1.png"),
  CarouselItem("Bitcoin only & Lightning fast.",
      "The highest level of security, at lightning speed.", "assets/carousel_2.png"),
  CarouselItem("Perpetual futures trading.",
      "Experience P2P leveraged trading with no counterparty risk.", "assets/carousel_3.png"),
  CarouselItem("Hedging and synthetics",
      "You can now send, receive and hold USDP natively on Lightning.", "assets/carousel_4.png"),
];

List<Widget> carouselItemWidgetLayers = [
  carouselItemWidget(caruselItems[0]),
  carouselItemWidget(caruselItems[1]),
  carouselItemWidget(caruselItems[2]),
  carouselItemWidget(caruselItems[3])
];

Widget carouselItemWidget(CarouselItem item) {
  return Stack(children: [
    Padding(
      padding: const EdgeInsets.fromLTRB(0, 0, 10, 0),
      child: ClipRRect(
          borderRadius: const BorderRadius.all(Radius.circular(5.0)),
          child: Image.asset(
            item.imagePath,
            fit: BoxFit.cover,
            width: 800.0,
          )),
    ),
    Padding(
      padding: const EdgeInsets.fromLTRB(15, 250, 15, 0),
      child: Text(
        item.title,
        style: const TextStyle(fontSize: 25, fontWeight: FontWeight.bold),
        textAlign: TextAlign.center,
      ),
    ),
    Padding(
      padding: const EdgeInsets.fromLTRB(15, 325, 15, 0),
      child: Text(
        item.description,
        style: const TextStyle(fontSize: 20),
        textAlign: TextAlign.center,
      ),
    )
  ]);
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
  bool buttonsDisabled = false;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        backgroundColor: Colors.white,
        body: ScrollableSafeArea(
            child: Container(
          color: Colors.white,
          padding: const EdgeInsets.all(20.0),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: <Widget>[
              const Spacer(),
              CarouselSlider(
                items: carouselItemWidgetLayers,
                carouselController: _controller,
                options: CarouselOptions(
                    scrollDirection: Axis.horizontal,
                    autoPlay: false,
                    enlargeCenterPage: true,
                    aspectRatio: 16 / 20,
                    padEnds: true,
                    enableInfiniteScroll: false,
                    onPageChanged: (index, reason) {
                      setState(() {
                        _current = index;
                      });
                    }),
              ),
              Row(
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
              const Spacer(),
              Column(children: [
                SizedBox(
                  width: 250,
                  child: ElevatedButton(
                      onPressed: buttonsDisabled
                          ? null
                          : () async {
                              setState(() {
                                buttonsDisabled = true;
                              });
                              final seedPath = await getSeedFilePath();
                              await api
                                  .initNewMnemonic(targetSeedFilePath: seedPath)
                                  .then((value) async {
                                Preferences.instance.hasEmailAddress().then((value) => {
                                      if (value)
                                        {GoRouter.of(context).go(WalletScreen.route)}
                                      else
                                        {GoRouter.of(context).go(WelcomeScreen.route)}
                                    });
                              }).catchError((error) {
                                logger.e("Could not create seed", error: error);
                                showSnackBar(ScaffoldMessenger.of(rootNavigatorKey.currentContext!),
                                    "Failed to create seed: $error");
                                // In case there was an error and we did not go forward, we want to be able to click the button again.
                                setState(() {
                                  buttonsDisabled = false;
                                });
                              });
                            },
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
                    onPressed: buttonsDisabled
                        ? null
                        : () {
                            setState(() {
                              buttonsDisabled = true;
                              GoRouter.of(context).go(SeedPhraseImporter.route);
                              buttonsDisabled = false;
                            });
                          },
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