import 'dart:io';

import 'package:bitcoin_icons/bitcoin_icons.dart';
import 'package:card_swiper/card_swiper.dart';
import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:get_10101/common/color.dart';
import 'package:get_10101/common/domain/model.dart';
import 'package:get_10101/features/brag/github_service.dart';
import 'package:get_10101/features/trade/domain/direction.dart';
import 'package:get_10101/features/trade/domain/leverage.dart';
import 'package:get_10101/logger/logger.dart';
import 'package:path_provider/path_provider.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:screenshot/screenshot.dart';
import 'package:provider/provider.dart';
import 'package:share_plus/share_plus.dart';

class BragWidget extends StatefulWidget {
  final String title;
  final VoidCallback onClose;
  final Direction direction;
  final Leverage leverage;
  final Amount? pnl;
  final int? pnlPercent;
  final Usd entryPrice;

  const BragWidget(
      {super.key,
      required this.title,
      required this.onClose,
      required this.direction,
      required this.leverage,
      required this.pnl,
      required this.entryPrice,
      this.pnlPercent});

  @override
  State<BragWidget> createState() => _BragWidgetState();
}

class _BragWidgetState extends State<BragWidget> {
  ScreenshotController screenShotController = ScreenshotController();
  int selectedIndex = 0;
  var images = [
    "https://github.com/bonomat/memes/blob/main/images/laser_eyes_portrait.png?raw=true",
    "https://github.com/bonomat/memes/blob/main/images/leoardo_cheers_portrait.png?raw=true",
    "https://github.com/bonomat/memes/blob/main/images/do_something_portrait.png?raw=true",
    "https://github.com/bonomat/memes/blob/main/images/got_some_sats_portrait.png?raw=true",
    "https://github.com/bonomat/memes/blob/main/images/are_you_winning_son_always_have_been_portrait.png?raw=true"
  ];

  @override
  Widget build(BuildContext context) {
    final githubService = context.read<GitHubService>();
    double height = 337.5 * 0.9 + 30;
    double width = 270.0 * 0.9 + 30;
    return AlertDialog(
      title: Text(widget.title),
      content: SizedBox(
        height: height,
        width: width,
        child: Column(
          children: [
            SizedBox(
              width: width - 30,
              height: height - 30,
              child: Screenshot(
                controller: screenShotController,
                child: FutureBuilder(
                  future: githubService.fetchMemeImages(),
                  builder: (BuildContext context, AsyncSnapshot<List<Meme>> snapshot) {
                    if (!snapshot.hasData) {
                      return const SizedBox(
                          width: 50, height: 50, child: Center(child: CircularProgressIndicator()));
                    } else {
                      return MemeWidget(
                        images: snapshot.data!.map((item) => item.downloadUrl).toList(),
                        pnl: widget.pnl ?? Amount.zero(),
                        leverage: widget.leverage,
                        direction: widget.direction,
                        entryPrice: widget.entryPrice,
                        onIndexChange: (index) {
                          setState(() {
                            selectedIndex = index;
                          });
                        },
                        pnlPercent: widget.pnlPercent ?? 0,
                      );
                    }
                  },
                ),
              ),
            ),
            const SizedBox(
              height: 10,
            ),
            Container(
              color: Colors.transparent,
              child: SizedBox(
                height: 20,
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.center,
                  children: images.asMap().entries.map((entry) {
                    final index = entry.key;
                    return Padding(
                      padding: const EdgeInsets.only(left: 8.0, right: 8.0),
                      child: Container(
                        decoration: BoxDecoration(
                          color: index == selectedIndex ? tenTenOnePurple : Colors.white,
                          borderRadius: BorderRadius.circular(50),
                          border: Border.all(color: tenTenOnePurple, width: 1),
                        ),
                        width: 10,
                        height: 10,
                      ),
                    );
                  }).toList(),
                ),
              ),
            )
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(context, 'Cancel'),
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: () async {
            await screenShotController
                .capture(delay: const Duration(milliseconds: 10))
                .then((image) async {
              logger.i("taking foto");
              if (image != null) {
                final directory = await getApplicationDocumentsDirectory();
                final imagePath = await File('${directory.path}/image.png').create();
                await imagePath.writeAsBytes(image);
                await Share.shareXFiles(
                    [XFile(imagePath.path, mimeType: "image/x-png", bytes: image)]);
              }
            }).catchError((error) {
              logger.e("Failed at capturing screenshot", error: error);
            }).whenComplete(widget.onClose);
          },
          child: const Text('Share'),
        ),
      ],
    );
  }
}

class MemeWidget extends StatelessWidget {
  const MemeWidget({
    super.key,
    required this.images,
    required this.pnl,
    required this.leverage,
    required this.direction,
    required this.entryPrice,
    required this.onIndexChange,
    required this.pnlPercent,
  });

  final List<String> images;
  final Amount pnl;
  final int pnlPercent;
  final Leverage leverage;
  final Direction direction;
  final Usd entryPrice;
  final Function onIndexChange;

  @override
  Widget build(BuildContext context) {
    bool losing = pnl.sats.isNegative;

    const gradientColor0 = tenTenOnePurple;
    var gradient1 = const Color.fromRGBO(0, 250, 130, 0.20);
    var gradient2 = const Color.fromRGBO(0, 250, 130, 0.1);
    var pnlColor = Colors.green.shade200;

    if (losing) {
      gradient1 = const Color.fromRGBO(250, 99, 99, 0.30);
      gradient2 = const Color.fromRGBO(250, 99, 99, 0.1);
      pnlColor = Colors.red.shade400;
    }

    const secondaryTextHeading = TextStyle(
      color: Colors.white,
      fontSize: 8.0,
      decoration: TextDecoration.underline,
    );
    var secondaryTextValue =
        const TextStyle(color: Colors.white, fontWeight: FontWeight.bold, fontSize: 15.0);
    var primaryTextStypeValue = TextStyle(
      color: pnlColor,
      fontWeight: FontWeight.bold,
      fontSize: 30,
    );

    return Swiper(
        itemCount: images.length,
        pagination: null,
        control: const SwiperControl(color: Colors.transparent),
        onIndexChanged: (index) {
          onIndexChange(index);
        },
        itemBuilder: (BuildContext context, int index) {
          return Container(
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(5.0),
            ),
            child: Stack(
              children: [
                Column(
                  children: [
                    Expanded(
                      child: Row(
                        children: [
                          Expanded(
                            flex: 3,
                            child: Container(
                              color: Colors.transparent,
                              child: Image.network(
                                images[index],
                                fit: BoxFit.fill,
                              ),
                            ),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
                Column(
                  children: [
                    Expanded(
                      flex: 3,
                      child: Container(
                          decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.bottomCenter,
                          end: Alignment.topCenter,
                          colors: [
                            gradient1,
                            gradient2,
                          ],
                        ),
                      )),
                    ),
                    Expanded(
                      flex: 2,
                      child: Container(
                          decoration: BoxDecoration(
                        gradient: LinearGradient(
                          begin: Alignment.bottomCenter,
                          end: Alignment.topCenter,
                          colors: [
                            gradientColor0,
                            gradient1,
                          ],
                        ),
                      )),
                    ),
                    Expanded(
                        flex: 2,
                        child: Container(
                          color: tenTenOnePurple,
                        )),
                  ],
                ),
                Align(
                    alignment: Alignment.topLeft,
                    child: SizedBox(
                      width: 55,
                      height: 55,
                      child: Padding(
                          padding: const EdgeInsets.all(8.0),
                          child: Container(
                            decoration: BoxDecoration(
                                color: Colors.white,
                                border: Border.all(
                                  color: tenTenOnePurple,
                                ),
                                borderRadius: const BorderRadius.all(Radius.circular(5))),
                            child: Padding(
                                padding: const EdgeInsets.all(2.0),
                                child: SvgPicture.asset(
                                  'assets/10101_logo.svg',
                                )),
                          )),
                    )),
                Align(
                  alignment: Alignment.topRight,
                  child: SizedBox(
                      width: 55,
                      height: 55,
                      child: Padding(
                        padding: const EdgeInsets.all(8.0),
                        child: Container(
                            decoration: BoxDecoration(
                                color: Colors.white,
                                border: Border.all(
                                  color: tenTenOnePurple,
                                ),
                                borderRadius: const BorderRadius.all(Radius.circular(5))),
                            child: FutureBuilder(
                                future: rust.api.referralStatus(),
                                builder: (BuildContext context,
                                    AsyncSnapshot<rust.ReferralStatus> snapshot) {
                                  if (!snapshot.hasData) {
                                    return const CircularProgressIndicator();
                                  }
                                  return QrImageView(
                                    data:
                                        "https://referral.10101.finance?referral=${snapshot.data!.referralCode}",
                                    eyeStyle: const QrEyeStyle(
                                      eyeShape: QrEyeShape.square,
                                      color: Colors.black,
                                    ),
                                    dataModuleStyle: const QrDataModuleStyle(
                                      dataModuleShape: QrDataModuleShape.square,
                                      color: Colors.black,
                                    ),
                                    version: QrVersions.auto,
                                    padding: const EdgeInsets.all(1),
                                  );
                                })),
                      )),
                ),
                Column(
                  mainAxisAlignment: MainAxisAlignment.end,
                  children: [
                    Padding(
                      padding: const EdgeInsets.only(top: 8.0, bottom: 8),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceEvenly,
                        children: [
                          Row(
                            children: [
                              FittedBox(
                                fit: BoxFit.scaleDown,
                                child: Text(
                                  pnl.formatted(),
                                  style: primaryTextStypeValue,
                                ),
                              ),
                              FittedBox(
                                fit: BoxFit.scaleDown,
                                child: Icon(
                                  BitcoinIcons.satoshi_v2,
                                  color: pnlColor,
                                  size: 20,
                                ),
                              ),
                            ],
                          ),
                          Row(
                            children: [
                              FittedBox(
                                fit: BoxFit.scaleDown,
                                child: Text(
                                  pnlPercent.toString(),
                                  style: primaryTextStypeValue,
                                ),
                              ),
                              FittedBox(
                                fit: BoxFit.scaleDown,
                                child: Icon(
                                  Icons.percent,
                                  color: pnlColor,
                                  size: 20,
                                ),
                              ),
                            ],
                          )
                        ],
                      ),
                    ),
                    const SizedBox(
                      height: 10,
                    ),
                    Padding(
                      padding: const EdgeInsets.only(left: 8, bottom: 20),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Row(
                            children: [
                              const Text(
                                "Leverage",
                                style: secondaryTextHeading,
                              ),
                              const SizedBox(
                                width: 5,
                              ),
                              Text(
                                leverage.formattedReverse(),
                                style: secondaryTextValue,
                              )
                            ],
                          ),
                          Row(
                            children: [
                              Padding(
                                padding: const EdgeInsets.only(left: 8),
                                child: Row(
                                  children: [
                                    const Text(
                                      "Side",
                                      style: secondaryTextHeading,
                                    ),
                                    const SizedBox(
                                      width: 5,
                                    ),
                                    Text(
                                      direction.nameU,
                                      style: secondaryTextValue,
                                    )
                                  ],
                                ),
                              )
                            ],
                          ),
                        ],
                      ),
                    ),
                    const Padding(
                      padding: EdgeInsets.only(bottom: 8.0),
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Text(
                            "I’m trading self-custodial and without counterparty risk at 10101",
                            style: TextStyle(
                                color: Colors.white, fontSize: 7, fontWeight: FontWeight.bold),
                          )
                        ],
                      ),
                    )
                  ],
                )
              ],
            ),
          );
        });
  }
}
